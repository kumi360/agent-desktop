#import "cursor_overlay_chrome.h"

// The `--style orb` look: a glass teardrop whose needle tip is the hotspot, a soft
// light trail that follows its motion and retracts once it rests, a thin-ring
// click pulse, and a glass label pill. Everything here is reached only when
// `ADStyle()->look == ADLookOrb`; the stock arrow never touches it.

static const CGFloat ADOrbDiameter = 15.2;  // bulb diameter
static const CGFloat ADOrbBleed = 26.0;
static const double ADOrbTrailLife = 0.21;
#define ADOrbTrailMax 96u
static const CGFloat ADOrbTrailJump = 420.0;

static __strong CALayer *ADOrbRoot = nil;
static __strong NSWindow *ADOrbTrailWindow = nil;
static __strong CAShapeLayer *ADOrbTrailGlow = nil;
static __strong CAGradientLayer *ADOrbTrailCore = nil;
static __strong CAShapeLayer *ADOrbTrailCoreMask = nil;
static NSPoint ADOrbTrailPoints[ADOrbTrailMax];
static CFTimeInterval ADOrbTrailTimes[ADOrbTrailMax];
static NSUInteger ADOrbTrailCount = 0;
static __strong CALayer *ADOrbPulseRoot = nil;

static CGColorRef ADOrbRGBA(CGFloat r, CGFloat g, CGFloat b, CGFloat a) {
    return [NSColor colorWithSRGBRed:r / 255.0 green:g / 255.0 blue:b / 255.0 alpha:a].CGColor;
}

static void ADOrbFreeze(CALayer *layer) {
    layer.actions = @{
        @"transform" : NSNull.null,
        @"opacity" : NSNull.null,
        @"position" : NSNull.null,
        @"bounds" : NSNull.null,
        @"frame" : NSNull.null,
        @"hidden" : NSNull.null,
        @"path" : NSNull.null,
        @"shadowPath" : NSNull.null,
        @"startPoint" : NSNull.null,
        @"endPoint" : NSNull.null,
        @"colors" : NSNull.null,
        @"contentsScale" : NSNull.null,
    };
}

bool ADOrbActive(void) {
    return ADStyle()->look == ADLookOrb;
}

static CGFloat ADOrbScale(void) {
    CGFloat backing = NSScreen.mainScreen.backingScaleFactor;
    for (NSScreen *screen in NSScreen.screens) {
        backing = MAX(backing, screen.backingScaleFactor);
    }
    return MAX(backing, 1.0);
}

static void ADOrbSetScale(CALayer *layer, CGFloat scale) {
    layer.contentsScale = scale;
    layer.rasterizationScale = scale;
    for (CALayer *child in layer.sublayers) {
        ADOrbSetScale(child, scale);
    }
    if (layer.mask != nil) {
        ADOrbSetScale(layer.mask, scale);
    }
}

static CAGradientLayer *ADOrbRadial(CGRect frame, CGPoint start, CGFloat radius, NSArray *colors,
                                    NSArray *stops) {
    CAGradientLayer *layer = [CAGradientLayer layer];
    layer.type = kCAGradientLayerRadial;
    layer.frame = frame;
    layer.startPoint = start;
    layer.endPoint = CGPointMake(start.x + radius, start.y + radius);
    layer.colors = colors;
    layer.locations = stops;
    ADOrbFreeze(layer);
    return layer;
}

static CGPathRef ADOrbCircle(CGPoint center, CGFloat radius) {
    return CGPathCreateWithEllipseInRect(
        CGRectMake(center.x - radius, center.y - radius, radius * 2.0, radius * 2.0), NULL);
}

// The glass teardrop, in its own square box (y up, like the pointer layer).
// The sharp tip is the click hotspot; the round bulb trails down and to the
// right along the classic pointer axis.
static const CGFloat ADDropBox = 34.0;
static const CGFloat ADDropTipX = 4.0;
static const CGFloat ADDropTipY = 30.0;
static const CGFloat ADDropReach = 18.0;  // tip to bulb centre
static const CGFloat ADDropTilt = 28.0;   // degrees off vertical

static CGPoint ADDropTip(void) {
    return CGPointMake(ADDropTipX, ADDropTipY);
}

static CGPoint ADDropBulb(void) {
    CGFloat tilt = ADDropTilt * M_PI / 180.0;
    return CGPointMake(ADDropTipX + ADDropReach * sin(tilt), ADDropTipY - ADDropReach * cos(tilt));
}

/// The teardrop outline: two faintly concave flanks meeting in a needle tip,
/// closed by the bulb's arc. `grow` offsets the bulb radius.
static CGPathRef ADDropPath(CGFloat grow) {
    CGPoint tip = ADDropTip();
    CGPoint bulb = ADDropBulb();
    CGFloat radius = ADOrbDiameter * 0.5 + grow;
    CGFloat toTip = atan2(tip.y - bulb.y, tip.x - bulb.x);
    CGFloat spread = acos(MIN(radius / ADDropReach, 1.0));
    CGFloat a = toTip + spread;
    CGFloat b = toTip - spread;
    CGPoint pa = CGPointMake(bulb.x + radius * cos(a), bulb.y + radius * sin(a));
    CGPoint pb = CGPointMake(bulb.x + radius * cos(b), bulb.y + radius * sin(b));
    CGPoint axis = CGPointMake((tip.x + bulb.x) * 0.5, (tip.y + bulb.y) * 0.5);
    CGFloat pinch = 0.16;
    CGPoint ca = CGPointMake((tip.x + pa.x) * 0.5 * (1 - pinch) + axis.x * pinch,
                             (tip.y + pa.y) * 0.5 * (1 - pinch) + axis.y * pinch);
    CGPoint cb = CGPointMake((tip.x + pb.x) * 0.5 * (1 - pinch) + axis.x * pinch,
                             (tip.y + pb.y) * 0.5 * (1 - pinch) + axis.y * pinch);
    CGMutablePathRef path = CGPathCreateMutable();
    CGPathMoveToPoint(path, NULL, tip.x, tip.y);
    CGPathAddQuadCurveToPoint(path, NULL, ca.x, ca.y, pa.x, pa.y);
    CGPathAddArc(path, NULL, bulb.x, bulb.y, radius, a, b + 2.0 * M_PI, false);
    CGPathAddQuadCurveToPoint(path, NULL, cb.x, cb.y, tip.x, tip.y);
    CGPathCloseSubpath(path);
    return path;
}

static CAShapeLayer *ADDropShape(CGPathRef path, CGRect frame) {
    CAShapeLayer *shape = [CAShapeLayer layer];
    shape.frame = frame;
    shape.path = path;
    shape.lineJoin = kCALineJoinMiter;
    shape.miterLimit = 12.0;
    ADOrbFreeze(shape);
    return shape;
}

/// Where the trail should flow from, relative to the hotspot, in points
/// (y up): the bulb's centre, so the trail streams out of the glass body.
static NSPoint ADOrbTrailOffset(void) {
    CGFloat size = ADStyle()->size;
    CGPoint bulb = ADDropBulb();
    return NSMakePoint((bulb.x - ADDropTipX) * size, (bulb.y - ADDropTipY) * size);
}

/// Builds the glass teardrop with its tip exactly on `hotspot` in the pointer
/// layer's space, so the pointer's size transform, and the click press,
/// scale it about the click point. Every soft effect is a gradient, not a
/// layer shadow, so it renders the same on screen and in captures.
CALayer *ADOrbLayer(CGPoint hotspot) {
    const CGFloat box = ADDropBox;
    const CGFloat r = ADOrbDiameter * 0.5;
    const CGPoint tip = ADDropTip();
    const CGPoint bulb = ADDropBulb();
    const CGRect bounds = CGRectMake(0.0, 0.0, box, box);
    CALayer *root = [CALayer layer];
    root.bounds = bounds;
    root.anchorPoint = CGPointMake(tip.x / box, tip.y / box);
    root.position = hotspot;
    root.hidden = YES;
    ADOrbFreeze(root);
    CGPathRef drop = ADDropPath(0.0);

    // Halo: cyan light around the bulb, for dark windows.
    CGFloat glow = r * 4.2;
    CGFloat rimAt = (r * 2.0) / glow;
    [root addSublayer:ADOrbRadial(CGRectMake(bulb.x - glow * 0.5, bulb.y - glow * 0.5, glow, glow),
                                  CGPointMake(0.5, 0.5), 0.5,
                                  @[
                                      (__bridge id)ADOrbRGBA(90, 226, 255, 0.10),
                                      (__bridge id)ADOrbRGBA(70, 220, 255, 0.40),
                                      (__bridge id)ADOrbRGBA(60, 214, 255, 0.12),
                                      (__bridge id)ADOrbRGBA(60, 214, 255, 0.0),
                                  ],
                                  @[ @0.0, @(rimAt), @(rimAt + 0.16), @1.0 ])];
    // A small cool glow just around the tip so the point reads on dark.
    [root addSublayer:ADOrbRadial(CGRectMake(tip.x - 4.5, tip.y - 4.5, 9.0, 9.0),
                                  CGPointMake(0.5, 0.5), 0.5,
                                  @[
                                      (__bridge id)ADOrbRGBA(200, 248, 255, 0.30),
                                      (__bridge id)ADOrbRGBA(90, 226, 255, 0.0),
                                  ],
                                  @[ @0.0, @1.0 ])];

    // Contact edge: a dark outline just outside the glass, offset down a
    // touch, so the shape stays crisp on white. The body covers its inner half.
    CAShapeLayer *contact = ADDropShape(drop, CGRectOffset(bounds, 0.0, -0.6));
    contact.fillColor = NSColor.clearColor.CGColor;
    contact.strokeColor = ADOrbRGBA(0, 22, 40, 0.40);
    contact.lineWidth = 2.0;
    [root addSublayer:contact];

    // Glass body: white light gathered at the needle tip, thinning along the
    // axis into clear cyan glass in the bulb.
    CGFloat tilt = ADDropTilt * M_PI / 180.0;
    CGPoint tail = CGPointMake(bulb.x + r * sin(tilt), bulb.y - r * cos(tilt));
    CAGradientLayer *body = [CAGradientLayer layer];
    body.frame = bounds;
    body.startPoint = CGPointMake(tip.x / box, tip.y / box);
    body.endPoint = CGPointMake(tail.x / box, tail.y / box);
    body.colors = @[
        (__bridge id)ADOrbRGBA(255, 255, 255, 0.97),
        (__bridge id)ADOrbRGBA(214, 248, 255, 0.60),
        (__bridge id)ADOrbRGBA(120, 220, 252, 0.30),
        (__bridge id)ADOrbRGBA(88, 206, 248, 0.22),
        (__bridge id)ADOrbRGBA(116, 226, 255, 0.36),
    ];
    body.locations = @[ @0.0, @0.13, @0.42, @0.74, @1.0 ];
    ADOrbFreeze(body);
    CAShapeLayer *bodyMask = ADDropShape(drop, bounds);
    bodyMask.fillColor = NSColor.whiteColor.CGColor;
    body.mask = bodyMask;
    [root addSublayer:body];

    // Fresnel: the glass thickens toward its edge all the way round, drawn as
    // two soft inner bands (the outer halves are clipped away).
    CALayer *fresnel = [CALayer layer];
    fresnel.frame = bounds;
    ADOrbFreeze(fresnel);
    CGFloat bands[2][2] = {{3.8, 0.26}, {1.9, 0.36}};
    for (int i = 0; i < 2; i += 1) {
        CAShapeLayer *band = ADDropShape(drop, bounds);
        band.fillColor = NSColor.clearColor.CGColor;
        band.strokeColor = ADOrbRGBA(140, 232, 255, bands[i][1]);
        band.lineWidth = bands[i][0];
        [fresnel addSublayer:band];
    }
    CAShapeLayer *fresnelMask = ADDropShape(drop, bounds);
    fresnelMask.fillColor = NSColor.whiteColor.CGColor;
    fresnel.mask = fresnelMask;
    [root addSublayer:fresnel];

    // Luminous core, slightly below the bulb's centre like focused light.
    CGFloat d = r * 2.0;
    CGRect bulbBox = CGRectMake(bulb.x - r, bulb.y - r, d, d);
    [root addSublayer:ADOrbRadial(bulbBox, CGPointMake(0.53, 0.42), 0.34,
                                  @[
                                      (__bridge id)ADOrbRGBA(240, 254, 255, 0.90),
                                      (__bridge id)ADOrbRGBA(140, 238, 255, 0.38),
                                      (__bridge id)ADOrbRGBA(140, 238, 255, 0.0),
                                  ],
                                  @[ @0.0, @0.35, @1.0 ])];
    // Inner refraction: a caustic on the far, lower-right wall of the bulb.
    [root addSublayer:ADOrbRadial(bulbBox, CGPointMake(0.74, 0.20), 0.30,
                                  @[
                                      (__bridge id)ADOrbRGBA(200, 252, 255, 0.78),
                                      (__bridge id)ADOrbRGBA(110, 232, 255, 0.0),
                                  ],
                                  @[ @0.0, @1.0 ])];

    // Specular: a thin sliver of window light along the upper-left flank,
    // running from the tip into the bulb.
    CGMutablePathRef sliver = CGPathCreateMutable();
    CGPathAddPath(sliver, NULL, drop);
    CGAffineTransform shift = CGAffineTransformMakeTranslation(1.25, -1.05);
    CGPathRef inner = ADDropPath(-0.9);
    CGPathAddPath(sliver, &shift, inner);
    CGPathRelease(inner);
    CAShapeLayer *crescent = ADDropShape(sliver, bounds);
    CGPathRelease(sliver);
    crescent.fillRule = kCAFillRuleEvenOdd;
    crescent.fillColor = NSColor.whiteColor.CGColor;
    CAGradientLayer *spec = [CAGradientLayer layer];
    spec.frame = bounds;
    spec.startPoint = CGPointMake(0.10, 0.90);
    spec.endPoint = CGPointMake(0.62, 0.22);
    spec.colors = @[
        (__bridge id)ADOrbRGBA(255, 255, 255, 0.58),
        (__bridge id)ADOrbRGBA(255, 255, 255, 0.0),
    ];
    spec.mask = crescent;
    ADOrbFreeze(spec);
    [root addSublayer:spec];
    // A soft glint on the bulb's shoulder.
    CGPoint glint = CGPointMake(bulb.x - r * 0.42, bulb.y + r * 0.40);
    [root addSublayer:ADOrbRadial(CGRectMake(glint.x - 2.3, glint.y - 1.9, 4.6, 3.8),
                                  CGPointMake(0.5, 0.5), 0.5,
                                  @[
                                      (__bridge id)ADOrbRGBA(255, 255, 255, 0.92),
                                      (__bridge id)ADOrbRGBA(255, 255, 255, 0.0),
                                  ],
                                  @[ @0.0, @1.0 ])];

    // Rim light: a hairline at the very edge, white at the tip and brightest
    // cyan around the bulb.
    CAGradientLayer *rim = [CAGradientLayer layer];
    rim.frame = bounds;
    rim.startPoint = CGPointMake(tip.x / box, tip.y / box);
    rim.endPoint = CGPointMake((bulb.x + r * 0.7) / box, (bulb.y - r * 0.7) / box);
    rim.colors = @[
        (__bridge id)ADOrbRGBA(255, 255, 255, 1.0),
        (__bridge id)ADOrbRGBA(170, 242, 255, 0.55),
        (__bridge id)ADOrbRGBA(110, 232, 255, 1.0),
    ];
    rim.locations = @[ @0.0, @0.55, @1.0 ];
    CAShapeLayer *ring = ADDropShape(drop, bounds);
    ring.fillColor = NSColor.clearColor.CGColor;
    ring.strokeColor = NSColor.whiteColor.CGColor;
    ring.lineWidth = 0.9;
    rim.mask = ring;
    ADOrbFreeze(rim);
    [root addSublayer:rim];

    // The white tip: a bright point of light right on the hotspot.
    CAGradientLayer *spark = ADOrbRadial(bounds, CGPointMake(tip.x / box + 0.035, tip.y / box - 0.06),
                                         4.6 / box,
                                         @[
                                             (__bridge id)ADOrbRGBA(255, 255, 255, 1.0),
                                             (__bridge id)ADOrbRGBA(255, 255, 255, 0.55),
                                             (__bridge id)ADOrbRGBA(255, 255, 255, 0.0),
                                         ],
                                         @[ @0.0, @0.45, @1.0 ]);
    CAShapeLayer *sparkMask = ADDropShape(drop, bounds);
    sparkMask.fillColor = NSColor.whiteColor.CGColor;
    spark.mask = sparkMask;
    [root addSublayer:spark];
    CGPathRelease(drop);

    ADOrbSetScale(root, ADOrbScale() * 2.0);
    ADOrbRoot = root;
    return root;
}

/// Shows the orb or the stock arrow layers, and rasterizes the orb sharply
/// for the current size multiplier.
void ADOrbApply(CALayer *orb, NSArray<CALayer *> *arrowLayers) {
    bool active = ADOrbActive();
    for (CALayer *layer in arrowLayers) {
        layer.hidden = active;
    }
    orb.hidden = !active;
    if (active) {
        ADOrbSetScale(orb, ADOrbScale() * MAX(ADStyle()->size, 1.0));
    } else {
        ADOrbTrailClear();
    }
}

static void ADOrbTrailEnsure(void) {
    if (ADOrbTrailWindow != nil) {
        return;
    }
    ADOrbTrailWindow = ADWindow(NSMakeRect(0.0, 0.0, 1.0, 1.0));
    ADOrbTrailWindow.level = NSPopUpMenuWindowLevel + 1;
    CALayer *stage = ADOrbTrailWindow.contentView.layer;

    ADOrbTrailGlow = [CAShapeLayer layer];
    ADOrbTrailGlow.fillColor = ADOrbRGBA(80, 222, 255, 0.20);
    ADOrbTrailGlow.strokeColor = NSColor.clearColor.CGColor;
    ADOrbTrailGlow.shadowColor = ADOrbRGBA(60, 214, 255, 1.0);
    ADOrbTrailGlow.shadowOpacity = 0.75;
    ADOrbTrailGlow.shadowRadius = 8.0;
    ADOrbTrailGlow.shadowOffset = CGSizeZero;
    ADOrbFreeze(ADOrbTrailGlow);
    [stage addSublayer:ADOrbTrailGlow];

    ADOrbTrailCore = [CAGradientLayer layer];
    ADOrbTrailCore.colors = @[
        (__bridge id)ADOrbRGBA(90, 226, 255, 0.0),
        (__bridge id)ADOrbRGBA(90, 226, 255, 0.42),
        (__bridge id)ADOrbRGBA(225, 251, 255, 0.85),
    ];
    ADOrbTrailCore.locations = @[ @0.0, @0.55, @1.0 ];
    ADOrbTrailCoreMask = [CAShapeLayer layer];
    ADOrbTrailCoreMask.fillColor = NSColor.whiteColor.CGColor;
    ADOrbFreeze(ADOrbTrailCoreMask);
    ADOrbTrailCore.mask = ADOrbTrailCoreMask;
    ADOrbFreeze(ADOrbTrailCore);
    [stage addSublayer:ADOrbTrailCore];
    ADOrbSetScale(stage, ADOrbScale());
}

/// A tapered ribbon through the trail points, `width(i)` wide at each.
static CGPathRef ADOrbRibbon(const NSPoint *points, const CGFloat *widths, NSUInteger count,
                             NSPoint origin) {
    CGMutablePathRef path = CGPathCreateMutable();
    NSPoint left[ADOrbTrailMax];
    NSPoint right[ADOrbTrailMax];
    for (NSUInteger i = 0; i < count; i += 1) {
        NSPoint a = points[i == 0 ? 0 : i - 1];
        NSPoint b = points[i + 1 < count ? i + 1 : count - 1];
        CGFloat dx = b.x - a.x;
        CGFloat dy = b.y - a.y;
        CGFloat length = hypot(dx, dy);
        CGFloat nx = length > 0.001 ? -dy / length : 0.0;
        CGFloat ny = length > 0.001 ? dx / length : 0.0;
        NSPoint p = NSMakePoint(points[i].x - origin.x, points[i].y - origin.y);
        left[i] = NSMakePoint(p.x + nx * widths[i], p.y + ny * widths[i]);
        right[i] = NSMakePoint(p.x - nx * widths[i], p.y - ny * widths[i]);
    }
    CGPathMoveToPoint(path, NULL, left[0].x, left[0].y);
    for (NSUInteger i = 1; i < count; i += 1) {
        NSPoint m = NSMakePoint((left[i - 1].x + left[i].x) * 0.5, (left[i - 1].y + left[i].y) * 0.5);
        CGPathAddQuadCurveToPoint(path, NULL, left[i - 1].x, left[i - 1].y, m.x, m.y);
    }
    CGPathAddLineToPoint(path, NULL, left[count - 1].x, left[count - 1].y);
    NSPoint head = NSMakePoint(points[count - 1].x - origin.x, points[count - 1].y - origin.y);
    CGFloat headWidth = widths[count - 1];
    CGFloat angle = atan2(left[count - 1].y - head.y, left[count - 1].x - head.x);
    CGPathAddArc(path, NULL, head.x, head.y, headWidth, angle, angle - M_PI, true);
    for (NSUInteger i = count - 1; i > 0; i -= 1) {
        NSPoint m =
            NSMakePoint((right[i].x + right[i - 1].x) * 0.5, (right[i].y + right[i - 1].y) * 0.5);
        CGPathAddQuadCurveToPoint(path, NULL, right[i].x, right[i].y, m.x, m.y);
    }
    CGPathAddLineToPoint(path, NULL, right[0].x, right[0].y);
    CGPathCloseSubpath(path);
    return path;
}

static void ADOrbTrailRender(CFTimeInterval now) {
    NSUInteger keep = 0;
    for (NSUInteger i = 0; i < ADOrbTrailCount; i += 1) {
        if (now - ADOrbTrailTimes[i] <= ADOrbTrailLife) {
            ADOrbTrailPoints[keep] = ADOrbTrailPoints[i];
            ADOrbTrailTimes[keep] = ADOrbTrailTimes[i];
            keep += 1;
        }
    }
    ADOrbTrailCount = keep;
    CGFloat span = 0.0;
    for (NSUInteger i = 1; i < ADOrbTrailCount; i += 1) {
        span += hypot(ADOrbTrailPoints[i].x - ADOrbTrailPoints[i - 1].x,
                      ADOrbTrailPoints[i].y - ADOrbTrailPoints[i - 1].y);
    }
    if (ADOrbTrailCount < 3 || span < 6.0) {
        [ADOrbTrailWindow orderOut:nil];
        return;
    }
    ADOrbTrailEnsure();
    CGFloat radius = ADOrbDiameter * 0.5 * ADStyle()->size;
    CGFloat glowWidths[ADOrbTrailMax];
    CGFloat coreWidths[ADOrbTrailMax];
    CGFloat minX = CGFLOAT_MAX, minY = CGFLOAT_MAX, maxX = -CGFLOAT_MAX, maxY = -CGFLOAT_MAX;
    for (NSUInteger i = 0; i < ADOrbTrailCount; i += 1) {
        CGFloat life = 1.0 - (now - ADOrbTrailTimes[i]) / ADOrbTrailLife;
        CGFloat along = (CGFloat)(i + 1) / (CGFloat)ADOrbTrailCount;
        CGFloat taper = pow(MAX(MIN(life, along), 0.0), 1.35);
        glowWidths[i] = MAX(radius * 0.78 * taper, 0.25);
        coreWidths[i] = MAX(radius * 0.36 * taper, 0.15);
        minX = MIN(minX, ADOrbTrailPoints[i].x);
        minY = MIN(minY, ADOrbTrailPoints[i].y);
        maxX = MAX(maxX, ADOrbTrailPoints[i].x);
        maxY = MAX(maxY, ADOrbTrailPoints[i].y);
    }
    NSRect frame = NSMakeRect(minX - ADOrbBleed, minY - ADOrbBleed,
                              maxX - minX + ADOrbBleed * 2.0, maxY - minY + ADOrbBleed * 2.0);
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    [ADOrbTrailWindow setFrame:frame display:NO];
    CGRect bounds = ADOrbTrailWindow.contentView.bounds;
    CGPathRef glow = ADOrbRibbon(ADOrbTrailPoints, glowWidths, ADOrbTrailCount, frame.origin);
    CGPathRef core = ADOrbRibbon(ADOrbTrailPoints, coreWidths, ADOrbTrailCount, frame.origin);
    ADOrbTrailGlow.frame = bounds;
    ADOrbTrailGlow.path = glow;
    ADOrbTrailGlow.shadowPath = glow;
    ADOrbTrailCore.frame = bounds;
    ADOrbTrailCoreMask.frame = bounds;
    ADOrbTrailCoreMask.path = core;
    NSPoint tail = ADOrbTrailPoints[0];
    NSPoint head = ADOrbTrailPoints[ADOrbTrailCount - 1];
    ADOrbTrailCore.startPoint =
        CGPointMake((tail.x - frame.origin.x) / frame.size.width,
                    (tail.y - frame.origin.y) / frame.size.height);
    ADOrbTrailCore.endPoint =
        CGPointMake((head.x - frame.origin.x) / frame.size.width,
                    (head.y - frame.origin.y) / frame.size.height);
    CGPathRelease(glow);
    CGPathRelease(core);
    [CATransaction commit];
    [ADOrbTrailWindow orderFrontRegardless];
}

/// Records where the hotspot is now (AppKit screen points) and redraws the
/// trail, which streams from the teardrop's bulb.
void ADOrbTrailPush(NSPoint point) {
    if (!ADOrbActive()) {
        return;
    }
    NSPoint offset = ADOrbTrailOffset();
    point = NSMakePoint(point.x + offset.x, point.y + offset.y);
    CFTimeInterval now = CACurrentMediaTime();
    if (ADOrbTrailCount > 0) {
        NSPoint last = ADOrbTrailPoints[ADOrbTrailCount - 1];
        CGFloat step = hypot(point.x - last.x, point.y - last.y);
        if (step > ADOrbTrailJump) {
            ADOrbTrailCount = 0;
        } else if (step < 0.5) {
            ADOrbTrailTimes[ADOrbTrailCount - 1] = now;
            ADOrbTrailRender(now);
            return;
        }
    }
    if (ADOrbTrailCount == ADOrbTrailMax) {
        memmove(ADOrbTrailPoints, ADOrbTrailPoints + 1, sizeof(NSPoint) * (ADOrbTrailMax - 1));
        memmove(ADOrbTrailTimes, ADOrbTrailTimes + 1,
                sizeof(CFTimeInterval) * (ADOrbTrailMax - 1));
        ADOrbTrailCount -= 1;
    }
    ADOrbTrailPoints[ADOrbTrailCount] = point;
    ADOrbTrailTimes[ADOrbTrailCount] = now;
    ADOrbTrailCount += 1;
    ADOrbTrailRender(now);
}

/// Called from the idle loop: lets the trail retract into the orb once it rests.
void ADOrbTrailTick(void) {
    if (ADOrbTrailCount == 0) {
        return;
    }
    ADOrbTrailRender(CACurrentMediaTime());
}

void ADOrbTrailClear(void) {
    ADOrbTrailCount = 0;
    [ADOrbTrailWindow orderOut:nil];
}

static CAShapeLayer *ADOrbPulseRing(void) {
    CAShapeLayer *ring = [CAShapeLayer layer];
    ring.frame = CGRectMake(0.0, 0.0, ADRippleSize, ADRippleSize);
    ring.fillColor = NSColor.clearColor.CGColor;
    ring.opacity = 0.0;
    ADOrbFreeze(ring);
    return ring;
}

static CAAnimationGroup *ADOrbRingAnimation(CGFloat from, CGFloat to, CGFloat width,
                                            CGFloat peak, double delay, double seconds) {
    CGPoint mid = CGPointMake(ADRippleSize * 0.5, ADRippleSize * 0.5);
    CGPathRef start = ADOrbCircle(mid, from);
    CGPathRef end = ADOrbCircle(mid, to);
    CABasicAnimation *grow = [CABasicAnimation animationWithKeyPath:@"path"];
    grow.fromValue = (__bridge id)start;
    grow.toValue = (__bridge id)end;
    CGPathRelease(start);
    CGPathRelease(end);
    CABasicAnimation *thin = [CABasicAnimation animationWithKeyPath:@"lineWidth"];
    thin.fromValue = @(width);
    thin.toValue = @(width * 0.3);
    CAKeyframeAnimation *fade = [CAKeyframeAnimation animationWithKeyPath:@"opacity"];
    fade.values = @[ @(peak), @(peak), @0.0 ];
    fade.keyTimes = @[ @0.0, @0.25, @1.0 ];
    CAAnimationGroup *group = [CAAnimationGroup animation];
    group.animations = @[ grow, thin, fade ];
    group.beginTime = CACurrentMediaTime() + delay;
    group.duration = seconds;
    group.fillMode = kCAFillModeBackwards;
    group.timingFunction = [CAMediaTimingFunction functionWithControlPoints:0.16:0.84:0.24:1.0];
    return group;
}

/// The click: two hairline rings open from the needle tip, a brief light
/// bloom on the hotspot, and the teardrop dips toward its tip as if pressed.
void ADOrbPulse(NSWindow *window) {
    CALayer *stage = window.contentView.layer;
    if (ADOrbPulseRoot == nil || ADOrbPulseRoot.superlayer != stage) {
        ADOrbPulseRoot = [CALayer layer];
        ADOrbPulseRoot.frame = stage.bounds;
        ADOrbFreeze(ADOrbPulseRoot);
        CGPoint mid = CGPointMake(ADRippleSize * 0.5, ADRippleSize * 0.5);
        CAGradientLayer *bloom =
            ADOrbRadial(CGRectMake(mid.x - 16.0, mid.y - 16.0, 32.0, 32.0), CGPointMake(0.5, 0.5),
                        0.5,
                        @[
                            (__bridge id)ADOrbRGBA(235, 252, 255, 0.85),
                            (__bridge id)ADOrbRGBA(90, 226, 255, 0.35),
                            (__bridge id)ADOrbRGBA(90, 226, 255, 0.0),
                        ],
                        @[ @0.0, @0.45, @1.0 ]);
        bloom.opacity = 0.0;
        [ADOrbPulseRoot addSublayer:bloom];
        CAShapeLayer *outer = ADOrbPulseRing();
        outer.strokeColor = ADOrbRGBA(232, 252, 255, 1.0);
        outer.shadowColor = ADOrbRGBA(60, 214, 255, 1.0);
        outer.shadowOpacity = 0.9;
        outer.shadowRadius = 3.0;
        outer.shadowOffset = CGSizeZero;
        [ADOrbPulseRoot addSublayer:outer];
        CAShapeLayer *inner = ADOrbPulseRing();
        inner.strokeColor = ADOrbRGBA(80, 224, 255, 1.0);
        [ADOrbPulseRoot addSublayer:inner];
        [stage addSublayer:ADOrbPulseRoot];
        ADOrbSetScale(ADOrbPulseRoot, ADOrbScale());
    }
    CGFloat size = ADStyle()->size;
    CGFloat radius = ADOrbDiameter * 0.5 * size;
    CGFloat reach = MIN(radius * 3.0, ADRippleSize * 0.5 - 3.0);
    NSArray<CALayer *> *layers = ADOrbPulseRoot.sublayers;
    [window orderFrontRegardless];
    for (CALayer *layer in layers) {
        [layer removeAllAnimations];
    }
    CALayer *bloom = layers[0];
    CAKeyframeAnimation *flash = [CAKeyframeAnimation animationWithKeyPath:@"opacity"];
    flash.values = @[ @0.0, @0.75, @0.0 ];
    flash.keyTimes = @[ @0.0, @0.2, @1.0 ];
    CABasicAnimation *swell = [CABasicAnimation animationWithKeyPath:@"transform.scale"];
    swell.fromValue = @(0.55 * size);
    swell.toValue = @(1.05 * size);
    CAAnimationGroup *bloomGroup = [CAAnimationGroup animation];
    bloomGroup.animations = @[ flash, swell ];
    bloomGroup.duration = 0.26;
    bloomGroup.timingFunction =
        [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseOut];
    [bloom addAnimation:bloomGroup forKey:@"agent-orb-bloom"];
    [layers[1] addAnimation:ADOrbRingAnimation(radius * 0.35, reach, 1.5, 0.95, 0.0, 0.46)
                     forKey:@"agent-orb-ring"];
    [layers[2] addAnimation:ADOrbRingAnimation(radius * 0.25, reach * 0.70, 1.0, 0.7, 0.07, 0.40)
                     forKey:@"agent-orb-ring"];
    if (ADOrbRoot != nil && !ADOrbRoot.hidden) {
        CAKeyframeAnimation *press = [CAKeyframeAnimation animationWithKeyPath:@"transform.scale"];
        press.values = @[ @1.0, @0.82, @1.06, @1.0 ];
        press.keyTimes = @[ @0.0, @0.28, @0.62, @1.0 ];
        press.duration = 0.24;
        press.timingFunction =
            [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseInEaseOut];
        [ADOrbRoot addAnimation:press forKey:@"agent-orb-press"];
    }
}

/// Restyles the label pill: dark glass with a cyan hairline in orb mode,
/// the stock black tag otherwise.
void ADOrbStylePill(CALayer *pill) {
    if (pill == nil) {
        return;
    }
    CALayer *sheen = pill.sublayers.firstObject;
    if (!ADOrbActive()) {
        pill.backgroundColor = NSColor.blackColor.CGColor;
        pill.borderWidth = 0.0;
        pill.shadowColor = NSColor.blackColor.CGColor;
        sheen.hidden = YES;
        return;
    }
    pill.backgroundColor = ADOrbRGBA(7, 22, 34, 0.80);
    pill.borderWidth = 0.75;
    pill.borderColor = ADOrbRGBA(124, 232, 255, 0.62);
    pill.shadowColor = ADOrbRGBA(0, 12, 22, 1.0);
    if (sheen == nil) {
        CAGradientLayer *gloss = [CAGradientLayer layer];
        gloss.colors = @[
            (__bridge id)ADOrbRGBA(255, 255, 255, 0.16),
            (__bridge id)ADOrbRGBA(255, 255, 255, 0.0),
        ];
        gloss.startPoint = CGPointMake(0.5, 1.0);
        gloss.endPoint = CGPointMake(0.5, 0.45);
        gloss.masksToBounds = YES;
        ADOrbFreeze(gloss);
        [pill addSublayer:gloss];
        sheen = gloss;
    }
    sheen.hidden = NO;
    sheen.frame = pill.bounds;
    sheen.cornerRadius = pill.cornerRadius;
    ADOrbSetScale(sheen, ADOrbScale());
}
