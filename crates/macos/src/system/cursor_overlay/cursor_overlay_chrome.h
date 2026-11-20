#pragma once

#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>

extern const CGFloat ADRippleSize;

typedef struct {
    double fill[3];
    double rim[3];
    double accent[3];
    double size;
    uint8_t look;
} AgentDesktopCursorStyle;

enum { ADLookArrow = 0, ADLookOrb = 1 };

CGColorRef ADColor(const double *rgb, CGFloat alpha);
const AgentDesktopCursorStyle *ADStyle(void);

NSWindow *ADWindow(NSRect frame);
void ADPump(NSApplication *app);
void ADFreezeLayer(CALayer *layer);
NSWindow *ADRippleWindow(void);
void ADRipplePlay(NSWindow *window);
void ADHighlightShow(NSRect frame, double seconds);
void ADHighlightStop(void);
void ADCaretShow(NSRect frame);
void ADCaretStop(void);
bool ADCaretShowing(void);
void ADTrailBegin(NSPoint point);
void ADTrailAppend(NSPoint point);
void ADTrailFinish(double seconds);
void ADTrailStop(void);
void ADShowBubble(NSTextField *text, NSRect frame, bool changed);
void ADPointerImageApply(NSWindow *window, CALayer *pointer);
void ADPointerImageSelect(NSWindow *window, CALayer *pointer, bool pointing);
void ADPointerImageRelease(NSWindow *window, CALayer *pointer, double seconds);
bool ADOrbActive(void);
CALayer *ADOrbLayer(CGPoint center);
void ADOrbApply(CALayer *orb, NSArray<CALayer *> *arrowLayers);
void ADOrbTrailPush(NSPoint point);
void ADOrbTrailTick(void);
void ADOrbTrailClear(void);
void ADOrbPulse(NSWindow *window);
void ADOrbStylePill(CALayer *pill);
