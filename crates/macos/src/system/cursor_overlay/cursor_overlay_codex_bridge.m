#import "cursor_overlay_chrome.h"

// The `--style codex` look (the default): a navigation-arrow pointer modelled on
// the Codex computer-use agent cursor, in Nimbus purple. A flat purple dart with
// a white rim sits in a soft Gaussian purple glow. While it travels the arrow
// turns to face where it is going, then settles back to the classic up-left
// pose. The tip is always the click hotspot. Measurements: ref/SPEC.md (t1023u).
// Everything here runs only when `ADStyle()->look == ADLookCodex`.

// Geometry in points at size 1.0, arrow pointing along -x with its tip at the
// origin (the hotspot). Outer rim edge: 14 pt tip to wing, 52 degree apex.
static const CGFloat ADCodexRim = 1.5;      // white rim width
static const CGFloat ADCodexRound = 0.5;    // corner rounding of the fill
static const CGFloat ADCodexWing = 12.0;    // tip to wing, path centreline
static const CGFloat ADCodexHalfApex = 26.0;
static const CGFloat ADCodexNotch = 0.78;   // notch depth as a share of wing depth
static const CGFloat ADCodexGlowSigma = 11.6;
static const CGFloat ADCodexGlowPeak = 0.86;  // luminance-matched to the reference glow
static const CGFloat ADCodexGlowReach = 36.0;
static const double ADCodexRestAngle = -45.0;  // degrees: tip up and to the left
static const double ADCodexSettleSeconds = 0.09;
static const double ADCodexTurnFollow = 0.38;   // per-frame share of the heading gap
static const double ADCodexRestFollow = 0.20;
static const CGFloat ADCodexMinStep = 0.6;
static const CGFloat ADCodexJump = 420.0;

static __strong CALayer *ADCodexRoot = nil;    // positioned on the tip, rotates
static __strong CALayer *ADCodexBody = nil;    // arrow + glow, pressed on click
static __strong CAGradientLayer *ADCodexGlow = nil;
static double ADCodexAngle = -45.0;
static NSPoint ADCodexLast = {0.0, 0.0};
static bool ADCodexHasLast = false;
static CFTimeInterval ADCodexLastMove = 0.0;

static CGColorRef ADCodexRGBA(CGFloat r, CGFloat g, CGFloat b, CGFloat a) {
    return [NSColor colorWithSRGBRed:r / 255.0 green:g / 255.0 blue:b / 255.0 alpha:a].CGColor;
}

// Nimbus purple. The glow is Anand's #9B5CFF; the fill is that hue at the
// Codex fill's saturation and value (its #1506EA blue, hue-rotated).
static CGColorRef ADCodexGlowColor(CGFloat alpha) {
    return ADCodexRGBA(155, 92, 255, alpha);
}

static CGColorRef ADCodexFillColor(void) {
    return ADCodexRGBA(101, 12, 235, 1.0);
}

static void ADCodexFreeze(CALayer *layer) {
    layer.actions = @{
        @"transform" : NSNull.null,
        @"opacity" : NSNull.null,
        @"position" : NSNull.null,
        @"bounds" : NSNull.null,
        @"frame" : NSNull.null,
        @"hidden" : NSNull.null,
        @"path" : NSNull.null,
        @"contentsScale" : NSNull.null,
    };
}

bool ADCodexActive(void) {
    return ADStyle()->look == ADLookCodex;
}

static CGFloat ADCodexBacking(void) {
    CGFloat backing = NSScreen.mainScreen.backingScaleFactor;
    for (NSScreen *screen in NSScreen.screens) {
        backing = MAX(backing, screen.backingScaleFactor);
    }
    return MAX(backing, 1.0);
}

static void ADCodexSetScale(CALayer *layer, CGFloat scale) {
    layer.contentsScale = scale;
    for (CALayer *child in layer.sublayers) {
        ADCodexSetScale(child, scale);
    }
}

/// The dart outline (tip, wing, notch, wing), shifted back along the axis so
/// the rounded outer rim, not the path vertex, lands on the hotspot.
static CGPathRef ADCodexDartPath(void) {
    CGFloat half = ADCodexHalfApex * M_PI / 180.0;
    CGFloat depth = ADCodexWing * cos(half);
    CGFloat spread = ADCodexWing * sin(half);
    // A round join pushes the outline `inset` out from the vertex on every
    // side, so the outer apex sits `inset` past the vertex; move the vertex back.
    CGFloat shift = ADCodexRim + ADCodexRound;
    CGMutablePathRef path = CGPathCreateMutable();
    CGPathMoveToPoint(path, NULL, shift, 0.0);
    CGPathAddLineToPoint(path, NULL, shift + depth, spread);
    CGPathAddLineToPoint(path, NULL, shift + depth * ADCodexNotch, 0.0);
    CGPathAddLineToPoint(path, NULL, shift + depth, -spread);
    CGPathCloseSubpath(path);
    return path;
}

/// The arrow's visual centre (where the glow is centred), in arrow space.
static CGPoint ADCodexCentre(void) {
    CGFloat half = ADCodexHalfApex * M_PI / 180.0;
    CGFloat depth = ADCodexWing * cos(half);
    CGFloat inset = ADCodexRim + ADCodexRound;
    return CGPointMake(inset + depth * (1.0 + 1.0 + ADCodexNotch) / 4.0, 0.0);
}

static CAShapeLayer *ADCodexShape(CGPathRef path, CGColorRef color, CGFloat width) {
    CAShapeLayer *shape = [CAShapeLayer layer];
    shape.frame = CGRectZero;
    shape.path = path;
    shape.fillColor = color;
    shape.strokeColor = color;
    shape.lineWidth = width;
    shape.lineJoin = kCALineJoinRound;
    ADCodexFreeze(shape);
    return shape;
}

static void ADCodexApplyAngle(void) {
    ADCodexRoot.transform = CATransform3DMakeRotation(ADCodexAngle * M_PI / 180.0, 0.0, 0.0, 1.0);
}

/// Builds the codex pointer, its origin pinned on `tip` (pointer-layer space).
CALayer *ADCodexLayer(CGPoint tip) {
    CALayer *root = [CALayer layer];
    root.bounds = CGRectZero;
    root.anchorPoint = CGPointZero;
    root.position = tip;
    root.hidden = YES;
    ADCodexFreeze(root);

    CALayer *body = [CALayer layer];
    body.bounds = CGRectZero;
    body.anchorPoint = CGPointZero;
    body.position = CGPointZero;
    ADCodexFreeze(body);
    [root addSublayer:body];

    // Gaussian glow: opacity = peak * exp(-r^2 / 2 sigma^2), sampled every 2 pt.
    CGPoint centre = ADCodexCentre();
    CAGradientLayer *glow = [CAGradientLayer layer];
    glow.type = kCAGradientLayerRadial;
    glow.frame = CGRectMake(centre.x - ADCodexGlowReach, centre.y - ADCodexGlowReach,
                            ADCodexGlowReach * 2.0, ADCodexGlowReach * 2.0);
    glow.startPoint = CGPointMake(0.5, 0.5);
    glow.endPoint = CGPointMake(1.0, 1.0);
    NSMutableArray *colors = [NSMutableArray array];
    NSMutableArray *stops = [NSMutableArray array];
    for (CGFloat r = 0.0; r <= ADCodexGlowReach + 0.01; r += 2.0) {
        CGFloat a = ADCodexGlowPeak * exp(-(r * r) / (2.0 * ADCodexGlowSigma * ADCodexGlowSigma));
        if (r >= ADCodexGlowReach - 0.01) {
            a = 0.0;
        }
        [colors addObject:(__bridge id)ADCodexGlowColor(a)];
        [stops addObject:@(r / ADCodexGlowReach)];
    }
    glow.colors = colors;
    glow.locations = stops;
    ADCodexFreeze(glow);
    [body addSublayer:glow];
    ADCodexGlow = glow;

    CGPathRef dart = ADCodexDartPath();
    CAShapeLayer *rim =
        ADCodexShape(dart, NSColor.whiteColor.CGColor, (ADCodexRim + ADCodexRound) * 2.0);
    rim.shadowColor = ADCodexRGBA(40, 10, 90, 1.0);
    rim.shadowOpacity = 0.28;
    rim.shadowRadius = 0.6;
    rim.shadowOffset = CGSizeZero;
    [body addSublayer:rim];
    [body addSublayer:ADCodexShape(dart, ADCodexFillColor(), ADCodexRound * 2.0)];
    CGPathRelease(dart);

    ADCodexRoot = root;
    ADCodexBody = body;
    ADCodexAngle = ADCodexRestAngle;
    ADCodexApplyAngle();
    ADCodexSetScale(root, ADCodexBacking() * 2.0);
    return root;
}

/// Shows the codex pointer or hands back to the other looks' layers.
void ADCodexApply(CALayer *codex, NSArray<CALayer *> *otherLayers) {
    bool active = ADCodexActive();
    if (active) {
        for (CALayer *layer in otherLayers) {
            layer.hidden = YES;
        }
        ADCodexSetScale(codex, ADCodexBacking() * MAX(ADStyle()->size, 1.0) * 2.0);
    }
    codex.hidden = !active;
}

static double ADCodexWrap(double degrees) {
    while (degrees > 180.0) degrees -= 360.0;
    while (degrees < -180.0) degrees += 360.0;
    return degrees;
}

static void ADCodexTurn(double target, double follow) {
    double gap = ADCodexWrap(target - ADCodexAngle);
    if (fabs(gap) < 0.05) {
        ADCodexAngle = target;
    } else {
        ADCodexAngle = ADCodexWrap(ADCodexAngle + gap * follow);
    }
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    ADCodexApplyAngle();
    [CATransaction commit];
}

/// Turns the arrow toward its travel direction. `point` is the hotspot in
/// AppKit screen points (y up).
void ADCodexMove(NSPoint point) {
    if (!ADCodexActive() || ADCodexRoot == nil) {
        return;
    }
    CFTimeInterval now = CACurrentMediaTime();
    if (!ADCodexHasLast) {
        ADCodexLast = point;
        ADCodexHasLast = true;
        ADCodexLastMove = now;
        return;
    }
    CGFloat dx = point.x - ADCodexLast.x;
    CGFloat dy = point.y - ADCodexLast.y;
    CGFloat step = hypot(dx, dy);
    if (step > ADCodexJump) {
        ADCodexLast = point;
        ADCodexLastMove = 0.0;
        return;
    }
    if (step < ADCodexMinStep) {
        return;
    }
    // Heading of travel; the arrow points along -x, so add 180 degrees.
    double heading = atan2(dy, dx) * 180.0 / M_PI;
    ADCodexTurn(ADCodexWrap(heading + 180.0), ADCodexTurnFollow);
    ADCodexLast = point;
    ADCodexLastMove = now;
}

/// Idle tick: once the arrow has stopped, ease it back to the up-left pose.
void ADCodexTick(void) {
    if (!ADCodexActive() || ADCodexRoot == nil) {
        return;
    }
    if (CACurrentMediaTime() - ADCodexLastMove < ADCodexSettleSeconds) {
        return;
    }
    if (fabs(ADCodexWrap(ADCodexRestAngle - ADCodexAngle)) < 0.05) {
        return;
    }
    ADCodexTurn(ADCodexRestAngle, ADCodexRestFollow);
}

/// Forgets the last position (on hide/stop) and snaps back to rest.
void ADCodexReset(void) {
    ADCodexHasLast = false;
    ADCodexLastMove = 0.0;
    if (ADCodexRoot != nil) {
        ADCodexAngle = ADCodexRestAngle;
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        ADCodexApplyAngle();
        [CATransaction commit];
    }
}

/// Click: a quick press of the arrow toward its tip and a brief glow lift.
/// No ripple ring, as in the reference.
void ADCodexPulse(NSWindow *window) {
    (void)window;
    if (ADCodexBody == nil) {
        return;
    }
    CABasicAnimation *press = [CABasicAnimation animationWithKeyPath:@"transform.scale"];
    press.fromValue = @1.0;
    press.toValue = @0.9;
    press.duration = 0.08;
    press.autoreverses = YES;
    press.timingFunction =
        [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseInEaseOut];
    [ADCodexBody addAnimation:press forKey:@"agent-codex-press"];
    CABasicAnimation *grow = [CABasicAnimation animationWithKeyPath:@"transform.scale"];
    grow.fromValue = @1.0;
    grow.toValue = @1.18;
    grow.duration = 0.08;
    grow.autoreverses = YES;
    grow.timingFunction =
        [CAMediaTimingFunction functionWithName:kCAMediaTimingFunctionEaseInEaseOut];
    [ADCodexGlow addAnimation:grow forKey:@"agent-codex-glow"];
}
