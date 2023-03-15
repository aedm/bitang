use crate::control::spline::{Spline, SplinePoint};
use egui::plot::{Legend, Line, Plot, PlotBounds};
use egui::{emath, Color32, InputState, Response};
use glam::Vec2;
use std::cell::RefCell;
use std::rc::Rc;

enum SplineEditorState {
    Idle,
    Drag,
    PointMove { index: usize },
}

pub struct SplineEditor {
    center_y: f32,
    min_x: f32,
    zoom: Vec2,
    state: SplineEditorState,
    spline: Option<Rc<RefCell<Spline>>>,
}

impl SplineEditor {
    pub fn new() -> Self {
        let mut spline = Spline::new();
        spline.points.push(SplinePoint {
            time: 0.0,
            value: 0.0,
            is_linear_after: false,
        });
        spline.points.push(SplinePoint {
            time: 3.0,
            value: 5.0,
            is_linear_after: false,
        });
        spline.points.push(SplinePoint {
            time: 7.0,
            value: -2.0,
            is_linear_after: false,
        });

        Self {
            center_y: 0.0,
            min_x: -2.0,
            zoom: Vec2::new(1.0, 1.0),
            state: SplineEditorState::Idle,
            spline: Some(Rc::new(RefCell::new(spline))),
        }
    }

    pub fn set_spline(&mut self, spline: &Rc<RefCell<Spline>>) {
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
            .show_x(false)
            .show_y(false)
            .include_x(self.min_x as f64)
            .include_x(max_x as f64)
            .include_y(min_y as f64)
            .include_y(max_y as f64)
            .allow_boxed_zoom(false)
            .allow_drag(false)
            .allow_zoom(false)
            .allow_scroll(false);
        let mut hover_index = None;
        let x = plot.show(ui, |plot_ui| {
            plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                [self.min_x as f64, min_y as f64],
                [max_x as f64, max_y as f64],
            ));
            hover_index = self.draw_spline(plot_ui, pixel_width);
        });

        self.handle_events(&ui.input(), &x.response, &pixel_size, hover_index);
    }

    // Returns the index of the hovered point
    fn draw_spline(
        &mut self,
        plot_ui: &mut egui::plot::PlotUi,
        pixel_width: isize,
    ) -> Option<usize> {
        if self.spline.is_none() {
            return None;
        }
        let spline = self.spline.as_ref().unwrap().borrow();

        let pixel_size = Vec2::new(
            Self::calculate_pixel_size(self.zoom.x),
            Self::calculate_pixel_size(self.zoom.y),
        );
        let hover_size = 4.0;
        let (hover_xs, hover_ys) = (
            hover_size * pixel_size.x as f64,
            hover_size * pixel_size.y as f64,
        );

        // Find hovered point
        let hover_index = if plot_ui.plot_hovered() {
            plot_ui.pointer_coordinate().and_then(|c| {
                spline.points.iter().position(|p| {
                    c.x >= p.time as f64 - hover_xs
                        && c.x <= p.time as f64 + hover_xs
                        && c.y >= p.value as f64 - hover_ys
                        && c.y <= p.value as f64 + hover_ys
                })
            })
        } else {
            None
        };

        // Draw spline
        let points = (0..=pixel_width)
            .map(|x_screen| {
                let time = self.min_x + (x_screen as f32 * pixel_size.x);
                let value = spline.get_value(time);
                [time as f64, value as f64]
            })
            .collect::<Vec<_>>();
        let line = Line::new(points)
            .color(Color32::from_rgb(100, 200, 100))
            // .style(self.line_style)
            .name("circle");
        plot_ui.line(line);

        // Draw points
        for (index, point) in spline.points.iter().enumerate() {
            let x = point.time as f64;
            let y = point.value as f64;
            let points = vec![
                [x - hover_xs, y - hover_ys],
                [x + hover_xs, y - hover_ys],
                [x + hover_xs, y + hover_ys],
                [x - hover_xs, y + hover_ys],
                [x - hover_xs, y - hover_ys],
            ];
            let mut rect = Line::new(points);
            let rect = if Some(index) == hover_index {
                rect.color(Color32::from_rgb(255, 255, 255)).width(2.0)
            } else {
                rect.color(Color32::from_rgb(150, 150, 150)).width(1.0)
            };
            plot_ui.line(rect);
        }

        hover_index
    }

    fn handle_events(
        &mut self,
        input: &InputState,
        response: &egui::Response,
        pixel_size: &Vec2,
        hover_index: Option<usize>,
    ) {
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
                if let Some(index) = hover_index {
                    if input.pointer.primary_clicked() {
                        self.state = SplineEditorState::PointMove { index };
                    }
                }
            }
            SplineEditorState::Drag => {
                self.min_x -= input.pointer.delta().x * Self::calculate_pixel_size(self.zoom.x);
                self.center_y += input.pointer.delta().y * Self::calculate_pixel_size(self.zoom.y);
                if !input.pointer.secondary_down() {
                    self.state = SplineEditorState::Idle;
                }
            }
            SplineEditorState::PointMove { index } => {
                if let Some(spline) = self.spline.as_mut() {
                    let mut spline = spline.borrow_mut();
                    let point = spline.points.get_mut(index).unwrap();
                    point.time += input.pointer.delta().x * Self::calculate_pixel_size(self.zoom.x);
                    point.value -=
                        input.pointer.delta().y * Self::calculate_pixel_size(self.zoom.y);
                }
                if !input.pointer.primary_down() {
                    self.state = SplineEditorState::Idle;
                }
            }
        }
    }
}
