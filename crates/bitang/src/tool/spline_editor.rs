use crate::control::controls::Control;
use crate::control::spline::SplinePoint;
use crate::tool::app_state::AppState;
use egui::plot::{Line, Plot, PlotBounds, PlotPoint};
use egui::Color32;
use glam::Vec2;
use std::rc::Rc;
use std::sync::Arc;

enum SplineEditorState {
    Idle,
    Pan,
    PointMove { index: usize },
}

pub struct SplineEditor {
    center_y: f32,
    min_x: f32,
    zoom: Vec2,
    state: SplineEditorState,
    control: Option<Rc<Control>>,
    component_index: usize,
    selected_index: Option<usize>,
}

impl SplineEditor {
    pub fn new() -> Self {
        Self {
            center_y: 0.0,
            min_x: -2.0,
            zoom: Vec2::new(1.0, 1.0),
            state: SplineEditorState::Idle,
            control: None,
            component_index: 0,
            selected_index: None,
        }
    }

    pub fn set_control(&mut self, control: &Rc<Control>, component_index: usize) {
        self.control = Some(control.clone());
        self.component_index = component_index;
        self.selected_index = None;
        #[allow(clippy::single_match)]
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

    pub fn draw(&mut self, ui: &mut egui::Ui, app_state: &mut AppState) {
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

        ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
            let time = app_state.cursor_time;
            self.draw_info(ui, time);
            let plot = Plot::new("spline_editor")
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
            let mut pointer_coordinate = None;
            let plot_response = plot.show(ui, |plot_ui| {
                plot_ui.set_plot_bounds(PlotBounds::from_min_max(
                    [self.min_x as f64, min_y as f64],
                    [max_x as f64, max_y as f64],
                ));
                self.paint_time_cursor(plot_ui, time, screen_size);
                (hover_index, pointer_coordinate) = self.draw_spline(plot_ui, pixel_width);
            });

            self.handle_events(
                ui,
                &plot_response.response,
                &pixel_size,
                hover_index,
                pointer_coordinate,
                app_state,
            );
        });
    }

    // Info about on the top
    fn draw_info(&mut self, ui: &mut egui::Ui, time: f32) {
        ui.with_layout(egui::Layout::left_to_right(egui::Align::Min), |ui| {
            if let Some(control) = self.control.as_mut() {
                let components = &mut control.components.borrow_mut();
                let spline = &mut components[self.component_index].spline;

                // Add new point
                if ui
                    .button("Add point")
                    .on_hover_text("Adds a new point at the current time")
                    .clicked()
                {
                    let value = spline.get_value(time);

                    // Unwrap is safe: time is always a valid float
                    let res = spline
                        .points
                        .binary_search_by(|p| p.time.partial_cmp(&time).unwrap());

                    let index_after = match res {
                        Ok(index) => index,
                        Err(index) => index,
                    };

                    spline.points.insert(
                        index_after,
                        SplinePoint {
                            time,
                            value,
                            is_linear_after: false,
                            hold_after: false,
                        },
                    );
                    self.selected_index = Some(index_after);
                }

                // Point-specific controls
                if let Some(index) = self.selected_index {
                    if ui
                        .button("Remove point")
                        .on_hover_text("Removes the selected point")
                        .clicked()
                    {
                        spline.points.remove(index);
                        self.selected_index = if index > 0 { Some(index - 1) } else { None };
                    } else {
                        let point = &mut spline.points[index];
                        ui.horizontal(|ui| {
                            ui.label("Time:");
                            ui.add(
                                egui::DragValue::new(&mut point.time)
                                    .speed(0.01)
                                    .max_decimals(6),
                            );
                            ui.label("Value:");
                            ui.add(
                                egui::DragValue::new(&mut point.value)
                                    .speed(0.01)
                                    .max_decimals(6),
                            );
                            ui.checkbox(&mut point.is_linear_after, "Linear");
                            ui.checkbox(&mut point.hold_after, "Hold");
                        });
                    }
                }
            }
            ui.label(" ");
        });
    }

    fn paint_time_cursor(
        &self,
        plot_ui: &mut egui::plot::PlotUi,
        time: f32,
        screen_size: egui::Vec2,
    ) {
        // Draw time
        let time_dy = (screen_size.y * Self::calculate_pixel_size(self.zoom.y)) as f64;
        let points = vec![
            [time as f64, self.center_y as f64 - time_dy],
            [time as f64, self.center_y as f64 + time_dy],
        ];
        plot_ui.line(
            Line::new(points)
                .color(Color32::from_rgb(150, 0, 150))
                .width(1.0),
        );
    }

    // Returns the index of the hovered point
    fn draw_spline(
        &mut self,
        plot_ui: &mut egui::plot::PlotUi,
        pixel_width: isize,
    ) -> (Option<usize>, Option<PlotPoint>) {
        let pointer_coordinate = plot_ui.pointer_coordinate();

        let Some(control) = self.control.as_ref() else {
            return (None, pointer_coordinate);
        };
        let components = control.components.borrow();
        let spline = &components[self.component_index].spline;

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
        let hover_index = if let SplineEditorState::PointMove { index } = self.state {
            Some(index)
        } else if plot_ui.plot_hovered() {
            pointer_coordinate.and_then(|c| {
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
            let rect = Line::new(points).name("circle");
            let rect = if Some(index) == self.selected_index {
                rect.color(Color32::from_rgb(255, 255, 255)).width(2.0)
            } else if Some(index) == hover_index {
                rect.color(Color32::from_rgb(220, 220, 220)).width(1.0)
            } else {
                rect.color(Color32::from_rgb(150, 150, 150)).width(1.0)
            };
            plot_ui.line(rect);
        }

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

        (hover_index, pointer_coordinate)
    }

    fn handle_events(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        pixel_size: &Vec2,
        hover_index: Option<usize>,
        pointer_coordinate: Option<PlotPoint>,
        app_state: &mut AppState,
    ) {
        let scroll_delta = ui.input(|i| i.scroll_delta);
        let zoom_delta = ui.input(|i| i.zoom_delta());
        let secondary_clicked = ui.input(|i| i.pointer.secondary_clicked());
        let primary_clicked = ui.input(|i| i.pointer.primary_clicked());
        let primary_down = ui.input(|i| i.pointer.primary_down());
        let secondary_down = ui.input(|i| i.pointer.secondary_down());
        let pointer_delta = ui.input(|i| i.pointer.delta());

        if let Some(hover) = response.hover_pos() {
            let hover = hover - response.rect.min;
            let screen_size = response.rect.size();

            // Horizontal zoom
            let zoom_x_delta = scroll_delta.y * -0.005;
            if zoom_x_delta != 0.0 {
                let plot_x = self.min_x + hover.x * pixel_size.x;
                self.zoom.x += zoom_x_delta;
                let pixel_size_x = Self::calculate_pixel_size(self.zoom.x);
                self.min_x = plot_x - hover.x * pixel_size_x;
            }

            // Vertical zoom
            let zoom_y_delta = (zoom_delta - 1.0) * -1.0;
            if zoom_y_delta != 0.0 {
                let plot_y = self.center_y - (hover.y - screen_size.y / 2.0) * pixel_size.y;
                self.zoom.y += zoom_y_delta;
                let pixel_size_y = Self::calculate_pixel_size(self.zoom.y);
                self.center_y = plot_y + (hover.y - screen_size.y / 2.0) * pixel_size_y;
            }
        }

        match self.state {
            SplineEditorState::Idle => {
                // Right click: pan
                if response.hovered() && secondary_clicked {
                    self.state = SplineEditorState::Pan;
                }

                // Left click: select point
                if primary_clicked {
                    if let Some(index) = hover_index {
                        self.state = SplineEditorState::PointMove { index };
                        self.selected_index = Some(index);
                    }
                }

                // Left click: set time if no point is selected
                if response.hovered() && primary_down && (!primary_clicked || hover_index.is_none())
                {
                    if let Some(pointer_coordinate) = pointer_coordinate {
                        app_state.set_time(pointer_coordinate.x as f32);
                        self.selected_index = None;
                    }
                }
            }
            SplineEditorState::Pan => {
                self.min_x -= pointer_delta.x * Self::calculate_pixel_size(self.zoom.x);
                self.center_y += pointer_delta.y * Self::calculate_pixel_size(self.zoom.y);
                if !secondary_down {
                    self.state = SplineEditorState::Idle;
                }
            }
            SplineEditorState::PointMove { index } => {
                if let Some(control) = self.control.as_mut() {
                    let mut components = control.components.borrow_mut();
                    let spline = &mut components[self.component_index].spline;
                    let point = spline.points.get_mut(index).unwrap();
                    point.time += pointer_delta.x * Self::calculate_pixel_size(self.zoom.x);
                    point.value -= pointer_delta.y * Self::calculate_pixel_size(self.zoom.y);
                }
                if !primary_down {
                    self.state = SplineEditorState::Idle;
                }
            }
        }
    }
}
