use std::cell::Cell;
use std::f64::consts::PI;
use std::rc::Rc;

use adw::prelude::*;
use gtk::glib::clone;
use gtk::glib;

/// A circular progress indicator with the percentage shown in the center,
/// similar to the one on Android's Quick Share. The displayed value animates
/// smoothly toward the target fraction instead of jumping.
pub struct CircularProgress {
    pub widget: gtk::Overlay,
    drawing: gtk::DrawingArea,
    label: gtk::Label,
    /// Where the ring currently is drawn (0.0..=1.0), animated.
    current: Rc<Cell<f64>>,
    /// Where we want it to go (0.0..=1.0).
    target: Rc<Cell<f64>>,
    /// Whether an animation tick is already scheduled.
    animating: Rc<Cell<bool>>,
}

impl CircularProgress {
    pub fn new(diameter: i32) -> Self {
        let current = Rc::new(Cell::new(0.0_f64));
        let target = Rc::new(Cell::new(0.0_f64));
        let animating = Rc::new(Cell::new(false));

        let drawing = gtk::DrawingArea::builder()
            .content_width(diameter)
            .content_height(diameter)
            .build();

        let label = gtk::Label::builder()
            .label("0%")
            .css_classes(["title-4", "accent"])
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();

        let overlay = gtk::Overlay::builder().child(&drawing).build();
        overlay.add_overlay(&label);

        // Draw the ring: a dim full circle + an accent arc for the progress.
        drawing.set_draw_func(clone!(
            #[strong]
            current,
            move |widget, cr, width, height| {
                let w = width as f64;
                let h = height as f64;
                let stroke = (w.min(h) * 0.11).max(4.0);
                let radius = (w.min(h) - stroke) / 2.0;
                let cx = w / 2.0;
                let cy = h / 2.0;

                // Resolve the theme's accent color (libadwaita 1.6+).
                let accent = adw::StyleManager::default().accent_color_rgba();
                let (ar, ag, ab) = (accent.red() as f64, accent.green() as f64, accent.blue() as f64);

                // Track (dim background ring).
                cr.set_line_width(stroke);
                cr.set_line_cap(gtk::cairo::LineCap::Round);
                cr.set_source_rgba(ar, ag, ab, 0.15);
                cr.arc(cx, cy, radius, 0.0, 2.0 * PI);
                let _ = cr.stroke();

                // Progress arc, starting at 12 o'clock, clockwise.
                let frac = current.get().clamp(0.0, 1.0);
                if frac > 0.0 {
                    let start = -PI / 2.0;
                    let end = start + 2.0 * PI * frac;
                    cr.set_source_rgba(ar, ag, ab, 1.0);
                    cr.arc(cx, cy, radius, start, end);
                    let _ = cr.stroke();
                }
                let _ = widget;
            }
        ));

        Self {
            widget: overlay,
            drawing,
            label,
            current,
            target,
            animating,
        }
    }

    /// Set the target progress (0.0..=1.0); the ring animates toward it.
    pub fn set_fraction(&self, fraction: f64) {
        let f = fraction.clamp(0.0, 1.0);
        self.target.set(f);
        self.label.set_label(&format!("{}%", (f * 100.0).round() as i32));

        if self.animating.get() {
            return; // a tick loop is already running; it will pick up the new target
        }
        self.animating.set(true);

        let current = self.current.clone();
        let target = self.target.clone();
        let animating = self.animating.clone();
        let drawing = self.drawing.clone();
        glib::timeout_add_local(
            std::time::Duration::from_millis(16), // ~60fps
            move || {
                let cur = current.get();
                let tgt = target.get();
                let diff = tgt - cur;
                if diff.abs() < 0.002 {
                    current.set(tgt);
                    drawing.queue_draw();
                    animating.set(false);
                    return glib::ControlFlow::Break;
                }
                // Ease toward the target.
                current.set(cur + diff * 0.18);
                drawing.queue_draw();
                glib::ControlFlow::Continue
            },
        );
    }

    /// Reset to zero without animation (e.g. starting a new transfer).
    pub fn reset(&self) {
        self.current.set(0.0);
        self.target.set(0.0);
        self.label.set_label("0%");
        self.drawing.queue_draw();
    }
}

/// Helper: build a circular progress and return it.
pub fn create_circular_progress(diameter: i32) -> CircularProgress {
    CircularProgress::new(diameter)
}
