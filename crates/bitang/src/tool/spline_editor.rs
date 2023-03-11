use crate::control::spline::Spline;
use egui::plot::{Legend, Line, Plot, PlotBounds};
use egui::{emath, Color32, InputState, Response};
use glam::Vec2;
use std::rc::Rc;

enum SplineEditorState {
    Idle,
    Drag,
    PointMove { index: usize, start: Vec2 },
}

pub struct SplineEditor {
    center_y: f32,
    min_x: f32,
    zoom: Vec2,
    state: SplineEditorState,
    spline: Option<Rc<Spline>>,
}

impl SplineEditor {
    pub fn new() -> Self {
        Self {
            center_y: 0.0,
            min_x: -2.0,
            zoom: Vec2::new(1.0, 1.0),
            state: SplineEditorState::Idle,
            spline: None,
        }
    }

    pub fn set_spline(&mut self, spline: &Rc<Spline>) {
        self.spline = Some(spline.clone());
        match self.state {
            SplineEditorState::PointMove { .. } => {
                self.state = SplineEditorState::Idle;
            }
            _ => {}
        }
    }

    fn calculate_pixel_size(zoom: f32) -> f32 {
        f32::exp(zoom) / 100.0
    }

    pub fn paint(&mut self, ui: &mut egui::Ui) {
        let pixel_width = ui.available_size().x.ceil() as isize;

        let screen_size = ui.available_size();
        let pixel_size = Vec2::new(
            Self::calculate_pixel_size(self.zoom.x),
            Self::calculate_pixel_size(self.zoom.y),
        );
        let plot_width = screen_size.x * pixel_size.x;
        let max_x = self.min_x + plot_width;
        let max_y = self.center_y + (screen_size.y * pixel_size.y) / 2.0;
        let min_y = self.center_y - (screen_size.y * pixel_size.y) / 2.0;

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
            self.draw_spline(plot_ui, pixel_width);
        });

        self.handle_events(&ui.input(), &x.response, &pixel_size);
    }

    fn draw_spline(&mut self, plot_ui: &mut egui::plot::PlotUi, pixel_width: isize) {
        if let Some(spline) = &self.spline {
            let spline = spline.as_ref();
        }
        let pixel_size = Vec2::new(
            Self::calculate_pixel_size(self.zoom.x),
            Self::calculate_pixel_size(self.zoom.y),
        );

        let points = (0..=pixel_width)
            .map(|x_screen| {
                let x = self.min_x + (x_screen as f32 * pixel_size.x);
                let y = x.sin();
                [x as f64, y as f64]
            })
            .collect::<Vec<_>>();
        let line = Line::new(points)
            .color(Color32::from_rgb(100, 200, 100))
            // .style(self.line_style)
            .name("circle");
        plot_ui.line(line);
    }

    fn handle_events(&mut self, input: &InputState, response: &egui::Response, pixel_size: &Vec2) {
        if let Some(hover) = response.hover_pos() {
            let hover = hover - response.rect.min;
            let screen_size = response.rect.size();

            // Horizontal zoom
            let zoom_x_delta = input.scroll_delta.y * -0.005;
            if zoom_x_delta != 0.0 {
                let plot_x = self.min_x + hover.x * pixel_size.x;
                self.zoom.x += zoom_x_delta;
                let pixel_size_x = Self::calculate_pixel_size(self.zoom.x);
                self.min_x = plot_x - hover.x * pixel_size_x;
            }

            // Vertical zoom
            let zoom_y_delta = (input.zoom_delta() - 1.0) * -1.0;
            if zoom_y_delta != 0.0 {
                let plot_y = self.center_y - (hover.y - screen_size.y / 2.0) * pixel_size.y;
                self.zoom.y += zoom_y_delta;
                let pixel_size_y = Self::calculate_pixel_size(self.zoom.y);
                self.center_y = plot_y + (hover.y - screen_size.y / 2.0) * pixel_size_y;
            }
        }

        match self.state {
            SplineEditorState::Idle => {
                if response.hovered() && input.pointer.secondary_clicked() {
                    self.state = SplineEditorState::Drag;
                }
            }
            SplineEditorState::Drag => {
                self.min_x -= input.pointer.delta().x * Self::calculate_pixel_size(self.zoom.x);
                self.center_y += input.pointer.delta().y * Self::calculate_pixel_size(self.zoom.y);
                if !input.pointer.secondary_down() {
                    self.state = SplineEditorState::Idle;
                }
            }
            _ => {}
        }
    }
}
