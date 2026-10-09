//! A hand-drawn vector icon set.
//!
//! The design uses a Lucide-flavoured stroke set on a 24px grid, which egui's
//! built-in font cannot supply. Every icon here is painted as geometry so the
//! chrome stays crisp at any DPI and never falls back to a tofu box.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

/// Signature of every icon: paint into `rect` in `color`.
pub type Icon = fn(&Painter, Rect, Color32);

/// Maps the 24×24 design grid onto an arbitrary rect.
struct Pen {
    painter: Painter,
    rect: Rect,
    color: Color32,
    width: f32,
}

impl Pen {
    fn new(painter: &Painter, rect: Rect, color: Color32, design_width: f32) -> Self {
        // The design is authored at 24px with a 1.6px stroke; keep that ratio.
        let width = (design_width * rect.width() / 24.0).max(1.0);
        Self {
            painter: painter.clone(),
            rect,
            color,
            width,
        }
    }

    fn at(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(
            self.rect.min.x + x / 24.0 * self.rect.width(),
            self.rect.min.y + y / 24.0 * self.rect.height(),
        )
    }

    fn stroke(&self) -> Stroke {
        Stroke::new(self.width, self.color)
    }

    fn poly(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|(x, y)| self.at(*x, *y)).collect();
        if points.len() >= 2 {
            self.painter.line(points, self.stroke());
        }
    }

    fn seg(&self, x1: f32, y1: f32, x2: f32, y2: f32) {
        self.painter
            .line_segment([self.at(x1, y1), self.at(x2, y2)], self.stroke());
    }

    fn circle(&self, cx: f32, cy: f32, r: f32) {
        self.painter
            .circle_stroke(self.at(cx, cy), r / 24.0 * self.rect.width(), self.stroke());
    }

    fn arc(&self, cx: f32, cy: f32, r: f32, from: f32, to: f32) {
        let steps = 16;
        let points: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let angle = (from + (to - from) * i as f32 / steps as f32).to_radians();
                self.at(cx + r * angle.cos(), cy + r * angle.sin())
            })
            .collect();
        self.painter.line(points, self.stroke());
    }

    /// A rounded rectangle outline.
    fn rrect(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let rect = Rect::from_min_size(self.at(x, y), Vec2::new(w, h) * self.scale());
        self.painter
            .rect_stroke(rect, r as u8, self.stroke(), egui::StrokeKind::Middle);
    }

    /// A filled rounded rectangle.
    fn rrect_fill(&self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        let rect = Rect::from_min_size(self.at(x, y), Vec2::new(w, h) * self.scale());
        self.painter.rect_filled(rect, r as u8, self.color);
    }

    fn scale(&self) -> f32 {
        self.rect.width() / 24.0
    }

    fn triangle(&self, points: &[(f32, f32)]) {
        let points: Vec<Pos2> = points.iter().map(|(x, y)| self.at(*x, *y)).collect();
        self.painter
            .add(Shape::convex_polygon(points, self.color, Stroke::NONE));
    }

    fn dot(&self, cx: f32, cy: f32) {
        self.painter
            .circle_filled(self.at(cx, cy), self.width * 1.15, self.color);
    }
}

macro_rules! stroke_icon {
    ($name:ident, |$pen:ident| $body:block) => {
        pub fn $name(painter: &Painter, rect: Rect, color: Color32) {
            let $pen = Pen::new(painter, rect, color, 1.6);
            $body
        }
    };
}

// -- navigation -------------------------------------------------------------

stroke_icon!(search, |p| {
    p.circle(11.0, 11.0, 7.0);
    p.seg(20.0, 20.0, 16.8, 16.8);
});

stroke_icon!(settings, |p| {
    p.circle(12.0, 12.0, 3.1);
    p.circle(12.0, 12.0, 7.0);
    for i in 0..8 {
        let angle = (i as f32 * 45.0).to_radians();
        let (sin, cos) = angle.sin_cos();
        p.seg(
            12.0 + 7.0 * cos,
            12.0 + 7.0 * sin,
            12.0 + 9.3 * cos,
            12.0 + 9.3 * sin,
        );
    }
});

stroke_icon!(chevron, |p| {
    p.poly(&[(6.6, 9.4), (12.0, 14.6), (17.4, 9.4)]);
});

stroke_icon!(close, |p| {
    p.seg(6.4, 6.4, 17.6, 17.6);
    p.seg(17.6, 6.4, 6.4, 17.6);
});

stroke_icon!(more, |p| {
    p.dot(6.4, 12.0);
    p.dot(12.0, 12.0);
    p.dot(17.6, 12.0);
});

// -- transport --------------------------------------------------------------

stroke_icon!(play, |p| {
    p.triangle(&[(8.4, 5.2), (19.2, 12.0), (8.4, 18.8)]);
});

stroke_icon!(pause, |p| {
    p.rrect_fill(7.0, 5.2, 3.4, 13.6, 1.4);
    p.rrect_fill(13.6, 5.2, 3.4, 13.6, 1.4);
});

stroke_icon!(prev, |p| {
    p.triangle(&[(17.8, 6.4), (9.2, 12.0), (17.8, 17.6)]);
    p.seg(6.8, 5.4, 6.8, 18.6);
});

stroke_icon!(next, |p| {
    p.triangle(&[(6.2, 6.4), (14.8, 12.0), (6.2, 17.6)]);
    p.seg(17.2, 5.4, 17.2, 18.6);
});

stroke_icon!(volume, |p| {
    p.poly(&[
        (11.0, 5.2),
        (6.6, 8.8),
        (3.9, 8.8),
        (3.9, 15.2),
        (6.6, 15.2),
        (11.0, 18.8),
        (11.0, 5.2),
    ]);
    p.arc(13.2, 12.0, 3.6, -52.0, 52.0);
    p.arc(12.0, 12.0, 6.9, -48.0, 48.0);
});

stroke_icon!(volume_muted, |p| {
    p.poly(&[
        (11.0, 5.2),
        (6.6, 8.8),
        (3.9, 8.8),
        (3.9, 15.2),
        (6.6, 15.2),
        (11.0, 18.8),
        (11.0, 5.2),
    ]);
    p.seg(15.5, 9.5, 20.5, 14.5);
    p.seg(20.5, 9.5, 15.5, 14.5);
});

// -- panels -----------------------------------------------------------------

stroke_icon!(queue, |p| {
    p.seg(4.0, 7.0, 14.0, 7.0);
    p.seg(4.0, 12.0, 14.0, 12.0);
    p.seg(4.0, 17.0, 10.0, 17.0);
    p.seg(17.5, 6.0, 17.5, 19.4);
    p.circle(17.5, 13.0, 1.6);
});

stroke_icon!(lyrics, |p| {
    p.poly(&[
        (5.5, 4.6),
        (18.5, 4.6),
        (19.9, 6.0),
        (19.9, 16.0),
        (18.5, 17.4),
        (11.6, 17.4),
        (7.2, 21.0),
        (7.2, 17.4),
        (5.5, 17.4),
        (4.1, 16.0),
        (4.1, 6.0),
        (5.5, 4.6),
    ]);
    p.seg(8.4, 9.2, 15.6, 9.2);
    p.seg(8.4, 12.6, 12.8, 12.6);
});

stroke_icon!(lock, |p| {
    p.rrect(4.8, 10.4, 14.4, 9.6, 2.0);
    p.poly(&[
        (8.4, 10.4),
        (8.4, 7.8),
        (9.5, 5.2),
        (12.0, 4.2),
        (14.5, 5.2),
        (15.6, 7.8),
        (15.6, 10.4),
    ]);
    p.seg(12.0, 14.4, 12.0, 16.2);
});

stroke_icon!(folder, |p| {
    p.poly(&[
        (4.0, 7.6),
        (5.8, 5.8),
        (8.9, 5.8),
        (10.7, 8.0),
        (18.2, 8.0),
        (20.0, 9.8),
        (20.0, 17.4),
        (18.2, 19.2),
        (5.8, 19.2),
        (4.0, 17.4),
        (4.0, 7.6),
    ]);
});

stroke_icon!(clock, |p| {
    p.circle(12.0, 12.0, 8.0);
    p.poly(&[(12.0, 7.6), (12.0, 12.0), (15.2, 14.0)]);
});

stroke_icon!(refresh, |p| {
    p.arc(12.0, 12.0, 8.0, -60.0, 240.0);
    p.poly(&[(20.0, 5.6), (20.0, 11.5), (14.1, 11.5)]);
});

stroke_icon!(check, |p| {
    p.poly(&[(5.4, 12.6), (9.7, 16.9), (18.6, 7.0)]);
});

stroke_icon!(copy, |p| {
    p.rrect(9.0, 9.0, 10.4, 10.4, 2.0);
    p.poly(&[
        (14.6, 6.2),
        (14.5, 4.6),
        (12.9, 4.6),
        (6.4, 4.6),
        (4.6, 6.4),
        (4.6, 12.9),
        (4.6, 14.5),
        (6.2, 14.6),
    ]);
});

stroke_icon!(qr, |p| {
    p.rrect(4.2, 4.2, 6.0, 6.0, 1.2);
    p.rrect(13.8, 4.2, 6.0, 6.0, 1.2);
    p.rrect(4.2, 13.8, 6.0, 6.0, 1.2);
    p.rrect_fill(14.0, 14.0, 2.2, 2.2, 0.4);
    p.rrect_fill(17.8, 17.8, 2.2, 2.2, 0.4);
    p.seg(13.8, 18.4, 13.8, 20.0);
    p.seg(18.6, 13.8, 20.0, 13.8);
});

stroke_icon!(translate, |p| {
    p.seg(4.4, 6.6, 11.8, 6.6);
    p.seg(8.1, 5.2, 8.1, 6.6);
    p.poly(&[(8.1, 6.6), (8.1, 8.6), (5.4, 13.0), (4.1, 14.0)]);
    p.poly(&[(6.3, 11.4), (8.4, 13.9), (10.9, 15.0)]);
    p.poly(&[(12.6, 19.4), (16.0, 10.8), (19.4, 19.4)]);
    p.seg(13.8, 16.6, 18.2, 16.6);
});

stroke_icon!(spark, |p| {
    // A four-point star, outlined rather than filled: at badge sizes the
    // silhouette reads better than a solid.
    p.poly(&[
        (12.0, 3.6),
        (13.6, 10.4),
        (20.4, 12.0),
        (13.6, 13.6),
        (12.0, 20.4),
        (10.4, 13.6),
        (3.6, 12.0),
        (10.4, 10.4),
        (12.0, 3.6),
    ]);
});

stroke_icon!(target, |p| {
    p.circle(12.0, 12.0, 7.6);
    p.circle(12.0, 12.0, 2.4);
    p.seg(12.0, 4.4, 12.0, 2.8);
    p.seg(12.0, 21.2, 12.0, 19.6);
    p.seg(4.4, 12.0, 2.8, 12.0);
    p.seg(21.2, 12.0, 19.6, 12.0);
});

/// The brand mark's three bars, drawn at absolute sizes rather than on the grid.
pub fn brand_bars(painter: &Painter, rect: Rect, color: Color32, scale: [f32; 3]) {
    let gap = rect.width() * 0.11;
    let bar_w = (rect.width() - gap * 2.0) / 3.0;
    for (index, factor) in scale.iter().enumerate() {
        let height = rect.height() * factor.clamp(0.1, 1.0);
        let x = rect.left() + index as f32 * (bar_w + gap);
        let bar = Rect::from_min_size(
            Pos2::new(x, rect.bottom() - height),
            Vec2::new(bar_w, height),
        );
        painter.rect_filled(bar, bar_w * 0.5, color);
    }
}
