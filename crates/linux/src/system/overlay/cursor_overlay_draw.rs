use agent_desktop_core::{
    AdapterError, CursorLook, CursorOverlayControl, CursorOverlayStyle, Point as CursorPoint,
};
use x11rb::connection::Connection;
use x11rb::protocol::shape::{ConnectionExt as _, SK, SO};
use x11rb::protocol::xproto::*;
use x11rb::rust_connection::RustConnection;
use x11rb::wrapper::ConnectionExt as _;

use super::error;

pub(super) struct Overlay {
    pub(super) connection: RustConnection,
    pub(super) window: Window,
    pub(super) point: CursorPoint,
}

impl Overlay {
    pub(super) fn open(display: &str) -> Result<Self, AdapterError> {
        let (connection, index) = super::connect(display)?;
        let screen = &connection.setup().roots[index];
        let argb_visual = super::visual::argb_visual(&connection, index)?;
        let visual = screen
            .allowed_depths
            .iter()
            .filter(|depth| depth.depth == 32)
            .flat_map(|depth| &depth.visuals)
            .find(|visual| {
                visual.class == VisualClass::TRUE_COLOR
                    && visual.visual_id == argb_visual
                    && visual.red_mask == 0xff0000
                    && visual.green_mask == 0xff00
                    && visual.blue_mask == 0xff
            })
            .ok_or_else(|| {
                AdapterError::not_supported("X11 cursor overlay needs an ARGB visual")
            })?;
        let colormap = connection.generate_id().map_err(error)?;
        connection
            .create_colormap(ColormapAlloc::NONE, colormap, screen.root, visual.visual_id)
            .map_err(error)?
            .check()
            .map_err(error)?;
        let window = connection.generate_id().map_err(error)?;
        connection
            .create_window(
                32,
                window,
                screen.root,
                0,
                0,
                32,
                40,
                0,
                WindowClass::INPUT_OUTPUT,
                visual.visual_id,
                &CreateWindowAux::new()
                    .colormap(colormap)
                    .border_pixel(0)
                    .background_pixel(0)
                    .override_redirect(1),
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .change_property8(
                PropMode::REPLACE,
                window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                b"agent-desktop cursor overlay",
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .shape_rectangles(
                SO::SET,
                SK::INPUT,
                ClipOrdering::UNSORTED,
                window,
                0,
                0,
                &[],
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        let pointer = connection
            .query_pointer(screen.root)
            .map_err(error)?
            .reply()
            .map_err(error)?;
        Ok(Self {
            connection,
            window,
            point: CursorPoint {
                x: f64::from(pointer.root_x),
                y: f64::from(pointer.root_y),
            },
        })
    }

    pub(super) fn update(&mut self, control: &CursorOverlayControl) -> Result<(), AdapterError> {
        let connection = &self.connection;
        if control.is_hide() {
            connection
                .unmap_window(self.window)
                .map_err(error)?
                .check()
                .map_err(error)?;
            return Ok(());
        }
        if let Some(instruction) = control.instruction() {
            self.point = instruction.destination().clone();
        }
        if let Some(style) = control.style() {
            self.paint(style)?;
        }
        connection
            .configure_window(
                self.window,
                &ConfigureWindowAux::new()
                    .x(self.point.x.round() as i32)
                    .y(self.point.y.round() as i32)
                    .stack_mode(StackMode::ABOVE),
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .map_window(self.window)
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .get_input_focus()
            .map_err(error)?
            .reply()
            .map_err(error)?;
        Ok(())
    }

    fn paint(&self, style: &CursorOverlayStyle) -> Result<(), AdapterError> {
        if !style.images().is_empty() {
            return Err(AdapterError::not_supported("custom cursor images on X11"));
        }
        if style.look() == CursorLook::Codex {
            return self.paint_codex(style.size());
        }
        let connection = &self.connection;
        let scale = style.size();
        let width = (24.0 * scale).ceil() as u16;
        let height = (32.0 * scale).ceil() as u16;
        let pixmap = connection.generate_id().map_err(error)?;
        connection
            .create_pixmap(32, pixmap, self.window, width, height)
            .map_err(error)?
            .check()
            .map_err(error)?;
        let gc = connection.generate_id().map_err(error)?;
        connection
            .create_gc(gc, pixmap, &CreateGCAux::new().foreground(0))
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .poly_fill_rectangle(
                pixmap,
                gc,
                &[Rectangle {
                    x: 0,
                    y: 0,
                    width,
                    height,
                }],
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        let polygon: Vec<Point> = [
            (0., 0.),
            (0., 24.),
            (6., 18.),
            (11., 29.),
            (16., 26.),
            (11., 16.),
            (21., 16.),
        ]
        .into_iter()
        .map(|(x, y)| Point {
            x: (x * scale) as i16,
            y: (y * scale) as i16,
        })
        .collect();
        let color = |rgb: [f64; 3]| {
            0xff000000
                | ((rgb[0] * 255.0).round() as u32) << 16
                | ((rgb[1] * 255.0).round() as u32) << 8
                | (rgb[2] * 255.0).round() as u32
        };
        connection
            .change_gc(gc, &ChangeGCAux::new().foreground(color(style.fill_rgb())))
            .map_err(error)?;
        connection
            .fill_poly(
                pixmap,
                gc,
                PolyShape::NONCONVEX,
                CoordMode::ORIGIN,
                &polygon,
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        let mut outline = polygon.clone();
        outline.push(polygon[0]);
        connection
            .change_gc(gc, &ChangeGCAux::new().foreground(color(style.rim_rgb())))
            .map_err(error)?;
        connection
            .poly_line(CoordMode::ORIGIN, pixmap, gc, &outline)
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .configure_window(
                self.window,
                &ConfigureWindowAux::new()
                    .width(u32::from(width))
                    .height(u32::from(height)),
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .change_window_attributes(
                self.window,
                &ChangeWindowAttributesAux::new().background_pixmap(pixmap),
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .shape_rectangles(
                SO::SET,
                SK::BOUNDING,
                ClipOrdering::UNSORTED,
                self.window,
                0,
                0,
                &silhouette(&polygon, height),
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .clear_area(false, self.window, 0, 0, 0, 0)
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .free_gc(gc)
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .free_pixmap(pixmap)
            .map_err(error)?
            .check()
            .map_err(error)?;
        Ok(())
    }

    /// Paints the codex look: a purple navigation dart with a white rim, tip up
    /// and to the left at the window origin. Its colours are fixed, so `--accent`,
    /// `--fill` and `--rim` never recolour it. X11 without a compositor cannot
    /// blend alpha, so the soft glow and the heading turn stay macOS only.
    fn paint_codex(&self, scale: f64) -> Result<(), AdapterError> {
        let connection = &self.connection;
        let side = (CODEX_SIDE * scale).ceil() as u16;
        let polygon = codex_dart(scale);
        let rim = ((CODEX_RIM + CODEX_ROUND) * 2.0 * scale).round().max(1.0) as u32;
        let round = ((CODEX_ROUND * 2.0 * scale).round().max(1.0)) as u32;
        let mut outline = polygon.clone();
        outline.push(polygon[0]);
        let pixmap = connection.generate_id().map_err(error)?;
        connection
            .create_pixmap(32, pixmap, self.window, side, side)
            .map_err(error)?
            .check()
            .map_err(error)?;
        let mask = connection.generate_id().map_err(error)?;
        connection
            .create_pixmap(1, mask, self.window, side, side)
            .map_err(error)?
            .check()
            .map_err(error)?;
        let gc = connection.generate_id().map_err(error)?;
        connection
            .create_gc(gc, pixmap, &CreateGCAux::new().foreground(0))
            .map_err(error)?
            .check()
            .map_err(error)?;
        let mask_gc = connection.generate_id().map_err(error)?;
        connection
            .create_gc(mask_gc, mask, &CreateGCAux::new().foreground(0))
            .map_err(error)?
            .check()
            .map_err(error)?;
        let all = Rectangle {
            x: 0,
            y: 0,
            width: side,
            height: side,
        };
        connection
            .poly_fill_rectangle(pixmap, gc, &[all])
            .map_err(error)?;
        connection
            .poly_fill_rectangle(mask, mask_gc, &[all])
            .map_err(error)?;
        let stroke = |drawable: Drawable, gc: Gcontext, colour: u32, width: u32| {
            connection
                .change_gc(
                    gc,
                    &ChangeGCAux::new()
                        .foreground(colour)
                        .line_width(width)
                        .join_style(JoinStyle::ROUND)
                        .cap_style(CapStyle::ROUND),
                )
                .map_err(error)?;
            connection
                .poly_line(CoordMode::ORIGIN, drawable, gc, &outline)
                .map_err(error)?;
            connection
                .fill_poly(
                    drawable,
                    gc,
                    PolyShape::NONCONVEX,
                    CoordMode::ORIGIN,
                    &polygon,
                )
                .map_err(error)?;
            Ok::<(), AdapterError>(())
        };
        stroke(mask, mask_gc, 1, rim)?;
        stroke(pixmap, gc, CODEX_RIM_COLOUR, rim)?;
        stroke(pixmap, gc, CODEX_FILL_COLOUR, round)?;
        connection
            .configure_window(
                self.window,
                &ConfigureWindowAux::new()
                    .width(u32::from(side))
                    .height(u32::from(side)),
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .change_window_attributes(
                self.window,
                &ChangeWindowAttributesAux::new().background_pixmap(pixmap),
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .shape_mask(SO::SET, SK::BOUNDING, self.window, 0, 0, mask)
            .map_err(error)?
            .check()
            .map_err(error)?;
        connection
            .clear_area(false, self.window, 0, 0, 0, 0)
            .map_err(error)?
            .check()
            .map_err(error)?;
        for gc in [gc, mask_gc] {
            connection
                .free_gc(gc)
                .map_err(error)?
                .check()
                .map_err(error)?;
        }
        for pixmap in [pixmap, mask] {
            connection
                .free_pixmap(pixmap)
                .map_err(error)?
                .check()
                .map_err(error)?;
        }
        Ok(())
    }

    pub(super) fn retain(self, owner: &[u8]) -> Result<(), AdapterError> {
        let atom = self
            .connection
            .intern_atom(false, super::OWNER)
            .map_err(error)?
            .reply()
            .map_err(error)?
            .atom;
        self.connection
            .change_property8(
                PropMode::REPLACE,
                self.window,
                atom,
                AtomEnum::STRING,
                owner,
            )
            .map_err(error)?
            .check()
            .map_err(error)?;
        self.connection
            .set_close_down_mode(CloseDown::RETAIN_PERMANENT)
            .map_err(error)?
            .check()
            .map_err(error)?;
        Ok(())
    }
}

fn silhouette(polygon: &[Point], height: u16) -> Vec<Rectangle> {
    (0..height as i16)
        .filter_map(|y| {
            let mut crossings: Vec<i16> = polygon
                .iter()
                .zip(polygon.iter().cycle().skip(1))
                .filter_map(|(a, b)| {
                    if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                        Some(
                            (f64::from(a.x)
                                + f64::from(y - a.y) * f64::from(b.x - a.x) / f64::from(b.y - a.y))
                            .round() as i16,
                        )
                    } else {
                        None
                    }
                })
                .collect();
            crossings.sort_unstable();
            Some(Rectangle {
                x: *crossings.first()?,
                y,
                width: (crossings.last()? - crossings.first()? + 1) as u16,
                height: 1,
            })
        })
        .collect()
}

/// The codex dart, matching the macOS bridge: wing 12, half apex 26 degrees,
/// notch 0.78 of the wing depth, at the rest heading of -45 degrees.
const CODEX_WING: f64 = 12.0;
const CODEX_HALF_APEX: f64 = 26.0;
const CODEX_NOTCH: f64 = 0.78;
const CODEX_RIM: f64 = 1.5;
const CODEX_ROUND: f64 = 0.5;
const CODEX_SIDE: f64 = 17.0;
/// #650CEB, the fill of the purple codex arrow.
pub(super) const CODEX_FILL_COLOUR: u32 = 0xff65_0ceb;
pub(super) const CODEX_RIM_COLOUR: u32 = 0xffff_ffff;

/// The dart's vertices in window pixels. The outer edge of the rim lands on the
/// window origin, which is the agent's point, like the macOS tip.
pub(super) fn codex_dart(scale: f64) -> Vec<Point> {
    let half = CODEX_HALF_APEX.to_radians();
    let depth = CODEX_WING * half.cos();
    let spread = CODEX_WING * half.sin();
    let shift = CODEX_RIM + CODEX_ROUND;
    let turn = std::f64::consts::FRAC_PI_4;
    [
        (0.0, 0.0),
        (depth, spread),
        (depth * CODEX_NOTCH, 0.0),
        (depth, -spread),
    ]
    .into_iter()
    .map(|(x, y): (f64, f64)| {
        let x = x + shift;
        let (rx, ry) = (
            x * turn.cos() - y * turn.sin(),
            x * turn.sin() + y * turn.cos(),
        );
        Point {
            x: (rx * scale).round() as i16,
            y: (ry * scale).round() as i16,
        }
    })
    .collect()
}
