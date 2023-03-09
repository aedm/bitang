use egui::plot::{Legend, Line, Plot, PlotBounds};
use egui::{emath, Color32};
use glam::Vec2;

enum SplineEditorModeState {
    Idle,
    Drag,
    PointMove { index: usize, start: Vec2 },
}

pub struct SplineEditor {
    center_y: f32,
    min_x: f32,
    zoom: Vec2,
    mode: SplineEditorModeState,
}

impl SplineEditor {
    pub fn new() -> Self {
        Self {
            center_y: 0.0,
            min_x: -2.0,
            zoom: Vec2::new(1.0, 1.0),
            mode: SplineEditorModeState::Idle,
        }
    }

    fn calculate_pixel_size(zoom: f32) -> f32 {
        f32::exp(zoom) / 100.0
    }

    fn screen_to_plot(screen_pos: f32, screen_size: f32, plot_min: f32, plot_size: f32) -> f32 {
        plot_min + (screen_pos / screen_size) * plot_size
    }

    pub fn paint(&mut self, ui: &mut egui::Ui) {
        let pixel_width = ui.available_size().x.ceil() as isize;

        let screen_size = ui.available_size();
        let pixel_size_x = Self::calculate_pixel_size(self.zoom.x);
        let pixel_size_y = Self::calculate_pixel_size(self.zoom.y);
        let plot_width = screen_size.x * pixel_size_x;
        let max_x = self.min_x + plot_width;
        let max_y = self.center_y + (screen_size.y * pixel_size_y) / 2.0;
        let min_y = self.center_y - (screen_size.y * pixel_size_y) / 2.0;

        ui.label("Spline editor");
        let mut plot = Plot::new("spline_editor")
            .legend(Legend::default())
            .include_x(self.min_x as f64)
            .include_x(max_x as f64)
            .include_y(min_y as f64)
            .include_y(max_y as f64)
            .allow_boxed_zoom(false)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false);
        let x = plot.show(ui, |plot_ui| {
            plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                [self.min_x as f64, min_y as f64],
                [max_x as f64, max_y as f64],
            ));
            let points = (0..=pixel_width)
                .map(|x_screen| {
                    let x = self.min_x + plot_width * (x_screen as f32 / pixel_width as f32);
                    let y = x.sin();
                    [x as f64, y as f64]
                })
                .collect::<Vec<_>>();
            let line = Line::new(points)
                .color(Color32::from_rgb(100, 200, 100))
                // .style(self.line_style)
                .name("circle");
            plot_ui.line(line);
        });

        if let Some(hover) = x.response.hover_pos() {
            let hover = hover - x.response.rect.min;

            // Horizontal zoom
            let zoom_x_delta = ui.input().scroll_delta.y * -0.005;
            if zoom_x_delta != 0.0 {
                let plot_x = Self::screen_to_plot(hover.x, screen_size.x, self.min_x, plot_width);
                self.zoom.x += zoom_x_delta;
                let pixel_size_x = Self::calculate_pixel_size(self.zoom.x);
                self.min_x = plot_x - (hover.x * pixel_size_x);
            }

            // Vertical zoom
            let zoom_y_delta = (ui.input().zoom_delta() - 1.0) * -1.0;
            if zoom_y_delta != 0.0 {
                let plot_y = Self::screen_to_plot(hover.y, screen_size.y, max_y, min_y - max_y);
                self.zoom.y += zoom_y_delta;
                let pixel_size_y = Self::calculate_pixel_size(self.zoom.y);
                self.center_y = plot_y + (hover.y - screen_size.y / 2.0) * pixel_size_y;
            }
        }

        match self.mode {
            SplineEditorModeState::Idle => {
                if x.response.hovered() && ui.input().pointer.secondary_clicked() {
                    self.mode = SplineEditorModeState::Drag;
                }
            }
            SplineEditorModeState::Drag => {
                self.min_x -= ui.input().pointer.delta().x * pixel_size_x;
                self.center_y += ui.input().pointer.delta().y * pixel_size_y;
                if !ui.input().pointer.secondary_down() {
                    self.mode = SplineEditorModeState::Idle;
                }
            }
            _ => {}
        }
    }
}
