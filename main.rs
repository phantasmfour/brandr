use eframe::{egui, App, epaint::ColorImage, epaint::TextureHandle, egui::TextureOptions};
use image::{DynamicImage, GenericImageView};
use std::process::Command;
use std::str;
use scrap::{Capturer, Display};
use std::time::{Duration, Instant};

mod cap;  // Import capture module I added in folder
struct Monitor {
    id: String,
    enabled: bool,
    orientation: String,
    resolution: (u32, u32),
    proposed_resolution: Option<(u32, u32)>, // Proposed resolution for the monitor
    position: egui::Pos2,
    initial_scaled_position: egui::Pos2,
    scale: f32,
    proposed_status: bool,
    screenshot: Option<DynamicImage>, // can be none with optional
    duplicate_of: Option<usize>, // Track which monitor is duplicated, if any
    last_screenshot_time: Instant,
    being_dragged: bool,  // If being dragged dont update the screenshot
    texture: Option<egui::TextureHandle>,
    scale_x_factor: f32,
    scale_y_factor: f32
}

struct MonitorApp {
    monitors: Vec<Monitor>,
    selected_monitor: Option<usize>, // Track the selected monitor
    drag_start: Option<usize>,       // Track the monitor being dragged
    screenshot_interval: Duration,
    net_zero_x: f32,
    net_zero_y: f32

    
}

impl Default for MonitorApp {  // Just defaults for your monitors
    // The monitors get updated but the other stuff doesn't really like the screenshot interval

    fn default() -> Self {
        let now = Instant::now();
        Self {
            monitors: vec![
                Monitor {
                    id: String::from(""),
                    enabled: true,
                    orientation: String::from("Landscape"),
                    resolution: (1920, 1080),
                    proposed_resolution: Some((1920,1080)), 
                    position: egui::Pos2::new(50.0, 50.0),
                    initial_scaled_position: egui::Pos2::new(0.0,0.0),
                    scale: 1.0,
                    proposed_status: false,
                    screenshot: None,
                    duplicate_of: None,
                    being_dragged: false,
                    last_screenshot_time: now,
                    texture: None,
                    scale_x_factor: 0.0,
                    scale_y_factor: 0.0,
                },
                Monitor {
                    id: String::from(""),
                    enabled: true,
                    orientation: String::from("Portrait"),
                    resolution: (1080, 1920),
                    proposed_resolution: Some((1920,1080)), 
                    position: egui::Pos2::new(300.0, 50.0),
                    initial_scaled_position: egui::Pos2::new(0.0,0.0),
                    scale: 1.0,
                    proposed_status: false,
                    screenshot: None,
                    duplicate_of: None,
                    being_dragged: false,
                    last_screenshot_time: now,
                    texture: None,
                    scale_x_factor: 0.0,
                    scale_y_factor: 0.0,
                },
            ],
            selected_monitor: None,
            drag_start: None,
            screenshot_interval: Duration::from_millis(5000), // can go fast but lags your pc. 1sec might be even fast
            net_zero_x: 0.0,
            net_zero_y: 0.0,
        }
    }
}

// Utility functions for MonitorApp
impl MonitorApp {

    fn draw_monitors(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        // The whole bounding box setup makes this pretty hard. Since you need to offset the rectangles to live in the box. And order them correctly.
        // If I redid anything it would be the logic here but not too much experience with dynmaic UIs and rust in general
        let bounding_box = egui::Rect::from_min_size(
            egui::Pos2::new(100.0, 100.0),
            egui::vec2(500.0, 300.0),
        );
        // Probably going to make this dynamic.

        // Precompute total width and height for scaling purposes
        let total_monitor_width: f32 = self.monitors.iter().map(|m| m.resolution.0 as f32).sum();
        let total_monitor_height: f32 = self.monitors.iter().map(|m| m.resolution.1 as f32).sum();

        let total_monitors = self.monitors.len();  // Used to divide by later
        let scale_factor_x: f32 = (bounding_box.width() / total_monitor_width) * 0.8;  // Scale the monitor to 80% of the bounding box.
        let scale_factor_y =(bounding_box.height() / total_monitor_height) * 0.8;


        
        let x_position_of_first_mon = (((bounding_box.size().x/scale_factor_x) - total_monitor_width)
                                                /total_monitors as f32) + (bounding_box.min.x/scale_factor_x);
        self.net_zero_x = x_position_of_first_mon; // Saved to use to check how far the distance is from normal?
        // Insanity, Take the real total mon width and subtract the space we would have in the bounding box if it was scaled up(we cannot use regular since it is used to scale, division reverses multiplication lol)
        // That gives you total space for monitors within the bounding box with some padding.
        // Divide the by the number of monitors as the outcome should give you the amount of padding to start with and end with
        // Then add the scaled bounding box starting x point since it starts somewhere and without it we would be starting on the edge of the screen.
        let mut x_position_of_next_mon = 0.0;  // If the monitor is not the first then we need to hold the position of the second monitor if there is a third
        // Since really the monitor in the corner could be any

        let mut first_mon_height = 0;

        if let Some(first_monitor) = self.monitors.first() {
            first_mon_height = first_monitor.resolution.1;  // Access height without owning
        }
        let y_position_of_first_mon = ((bounding_box.height()/scale_factor_y)/2.0 - (first_mon_height as f32 /2.0)) + (bounding_box.min.y/scale_factor_y);
        self.net_zero_y = y_position_of_first_mon;
        // More insanity but i call in minsanity
        // Get the height of the first mon. Hopefully even lol but will use first as the guide. 
        // divide the mon height by 2 to get the midpoint. 
        // Take the bounding box height divide by two to get the midpoint. 
        // box should be bigger than mon height subtract them to give you where the monitor height should start.
        // Then add the offset of where the boundingbox actually starts getting drawn. Same scale divides as x. This ones easiers though since all mons can use same y
        // All just used to center the monitors in the box.

        ui.painter().rect_stroke(
            bounding_box,
            0.0,
            egui::Stroke::new(1.0, egui::Color32::LIGHT_GRAY),
        ); // Painting of the bounding box. 

        let displays = scrap::Display::all().unwrap();  // Pull the the displays from scrap should be all that are connected
        let displays_size = displays.len();
        let mut wasMoved = 0;
        for (i, display) in displays.into_iter().enumerate() {
            let any_active = {
                self.monitors.iter_mut().any(|monitor| monitor.being_dragged)
            };// Rust not letting you borrow something more than once.. So use it to set a var then we can release it for the mut to get referenced again

            let mut monitor = &mut self.monitors[i+wasMoved]; // I think I take ownership of monitors here. 
            //dbg!(&monitor.id);
            if monitor.enabled == false {
                monitor = &mut self.monitors[i+1+wasMoved];
                wasMoved += 1;
                //dbg!(&monitor.id);
                //dbg!(wasMoved);
            }
            //dbg!("Pre Scaled Position");
            //dbg!(monitor.position);
            
            // The initial position needs to be added by the bounding box to be put in there as well
            // So on spawn adjust their position to be in the box.
            
            if monitor.texture == None {
                //dbg!("No texture");
                // if first monitor set position to center of bounding box and monitor offsets
                if monitor.position.x == 0.0 {  // First monitor
                    monitor.position.x = x_position_of_first_mon;
                    
                    monitor.position.y = y_position_of_first_mon;
                    monitor.initial_scaled_position = egui::Pos2::new(monitor.position.x * scale_factor_x,monitor.position.y * scale_factor_y);
                    // To check after if the monitors have been moved by the user later
                }
                // other monitors if x add them together to get correct spacing
                // y's all can stay the same.
                else{
                    // You could just be a new mon enabled without a screenshot.. This should really just be inital run. How can I say receently enabled. Eh I can just apply a default texture
                    //dbg!("not first mon");
                    monitor.position.x = x_position_of_first_mon + x_position_of_next_mon + (total_monitor_width/total_monitors as f32);
                    // Take the position where the first monitor started and a the size of the monitor. Should give us perfect aligntment
                    // And add the position of the next mon incase there is a third. 
                    x_position_of_next_mon = monitor.position.x;  // Incase you have more monitors keep it going. and save to seperate var incase the second mon was first in the output

                    monitor.position.y = y_position_of_first_mon; // all mons can use same y no need to change.
                    monitor.initial_scaled_position = egui::Pos2::new(monitor.position.x * scale_factor_x,monitor.position.y * scale_factor_y);
                    // to check after initial position if monitors have been moved by the user later.
                }

                // Set the scaled factor so we can undo it later to get the position.. Should change on new run
                monitor.scale_x_factor = scale_factor_x;
                monitor.scale_y_factor = scale_factor_y;

            } 
            
            
            let scaled_position = egui::Pos2::new(
                monitor.position.x * scale_factor_x,
                monitor.position.y * scale_factor_y,
                
            );
            // Forces me a bit to divide by the scale factor for this to all work correctly. to center things. Cannot just use real unscaled numbers to set the position since it would get multiplied by the scale factor and break the proportions.   
            let scaled_size = egui::vec2(
                monitor.resolution.0 as f32 * scale_factor_x,
                monitor.resolution.1 as f32 * scale_factor_y,
            );
            //dbg!(monitor.position);

            // Check if it's time to update the screenshot. If there is no texture we need. One. Really we need a default texture but will live.
            if (monitor.last_screenshot_time.elapsed() > self.screenshot_interval && (any_active == false)) || monitor.texture == None {
                if let Some(screenshot) = cap::capture_screen(display) {
                    monitor.last_screenshot_time = Instant::now();  // Reset the timer

                    // Convert the new screenshot to a texture
                    let (width, height) = screenshot.dimensions();
                    let color_image = ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        &screenshot.to_rgba(),
                    );

                    // Load the new texture
                    monitor.texture = Some(ctx.load_texture(
                        "monitor_screenshot",
                        color_image,
                        TextureOptions::default(),
                    ));
                }
            }
            

            // Render the texture if it exists
            if let Some(texture) = &monitor.texture {
                let monitor_rect = egui::Rect::from_min_size(
                    scaled_position,
                    scaled_size,
                    );
                let response = ui.allocate_rect(monitor_rect, egui::Sense::click_and_drag());
                
                // Keep monitors inside the bounding box and adjust positions if needed           
                if monitor_rect.min.x < bounding_box.min.x {
                    monitor.position.x = bounding_box.min.x / scale_factor_x;
                }
                if monitor_rect.max.x > bounding_box.max.x {
                    monitor.position.x = (bounding_box.max.x - scaled_size.x) / scale_factor_x;
                }
                if monitor_rect.min.y < bounding_box.min.y {
                    monitor.position.y = bounding_box.min.y / scale_factor_y;
                }
                if monitor_rect.max.y > bounding_box.max.y {
                    monitor.position.y = (bounding_box.max.y - scaled_size.y) / scale_factor_y;
                }
                
                
                

                if response.dragged() { // Move the monitor by the drag delta with the scale factor
                    monitor.position += response.drag_delta() / egui::Vec2::new(scale_factor_x, scale_factor_y);
                }
                        // Check if dragging and set flag
                if response.drag_started() {
                    monitor.being_dragged = true;  // Used for saying when to take a screenshot or not so we don't update when you are touching the screen
                }
                if response.drag_released() {
                    monitor.being_dragged = false; // Still some lag when clicking but helps a lot and don't have to thread
                }


                // When clicked select the monitor
                if response.clicked() {
                    self.selected_monitor = Some(i+wasMoved);
                }
                
            
                
                // Paint the screenshot within the monitor rectangle
                egui::Image::new(texture)
                    .rounding(5.0)
                    .tint(egui::Color32::WHITE)
                    .paint_at(ui, monitor_rect);
            }
        }
        

        // Render disabled displays
        // Second loop to render blank monitors that were not in scrap
        if self.monitors.len() - displays_size != 0 { 
            let max_x_position = self.monitors.iter()
                .filter(|monitor| monitor.enabled) // Only include enabled monitors
                .map(|monitor| monitor.position.x)
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0); // Default to 0.0 if no enabled monitors found
            // This is used because I need to know where the last monitor was. This breaks down though with multiple disabled monitors. Lots of bugs with this approach but shoudl work. 

            // There is a disabled mon
            for i in 0..self.monitors.len() {
                let mon = &mut self.monitors[i];

                if mon.enabled == false{
                    //dbg!(x_position_of_next_mon); // Math this tomorrow somethings wrong here not scaling to three mons if was empty or not. better to look above since I think the issue is there. debug by running quickly to see what this is at.
                    mon.position.x = max_x_position + (total_monitor_width/total_monitors as f32);
                    // Take the position where the first monitor started and a the size of the monitor. Should give us perfect aligntment
                    // And add the position of the next mon incase there is a third. 
                    //x_position_of_next_mon = mon.position.x;  // Incase you have more monitors keep it going. and save to seperate var incase the second mon was first in the output
                    mon.position.y = y_position_of_first_mon; // all mons can use same y no need to change.
                    mon.initial_scaled_position = egui::Pos2::new(mon.position.x * scale_factor_x,mon.position.y * scale_factor_y);
                    // to check after initial position if monitors have been moved by the user later.

                    let scaled_position = egui::Pos2::new(
                        mon.position.x * scale_factor_x,
                        mon.position.y * scale_factor_y,
                    );

                    let scaled_size = egui::vec2(
                        mon.resolution.0 as f32 * scale_factor_x,
                        mon.resolution.1 as f32 * scale_factor_y,
                    );
                    let monitor_rect = egui::Rect::from_min_size(scaled_position, scaled_size);
                    
                    let response = ui.allocate_rect(monitor_rect, egui::Sense::click_and_drag());
                        
                        // Keep monitors inside the bounding box and adjust positions if needed           
                        if monitor_rect.min.x < bounding_box.min.x {
                            mon.position.x = bounding_box.min.x / scale_factor_x;
                        }
                        if monitor_rect.max.x > bounding_box.max.x {
                            mon.position.x = (bounding_box.max.x - scaled_size.x) / scale_factor_x;
                        }
                        if monitor_rect.min.y < bounding_box.min.y {
                            mon.position.y = bounding_box.min.y / scale_factor_y;
                        }
                        if monitor_rect.max.y > bounding_box.max.y {
                            mon.position.y = (bounding_box.max.y - scaled_size.y) / scale_factor_y;
                        }
                        
                        
                        

                        if response.dragged() { // Move the monitor by the drag delta with the scale factor
                            mon.position += response.drag_delta() / egui::Vec2::new(scale_factor_x, scale_factor_y);
                        }
                                // Check if dragging and set flag
                        if response.drag_started() {
                            mon.being_dragged = true;  // Used for saying when to take a screenshot or not so we don't update when you are touching the screen
                        }
                        if response.drag_released() {
                            mon.being_dragged = false; // Still some lag when clicking but helps a lot and don't have to thread
                        }
                        // When clicked select the monitor
                        if response.clicked() {
                            self.selected_monitor = Some(i);
                        }
                    // Render blank monitor with a gray fill
                    let gray_image = vec![128u8; 10 * 10 * 4]; // RGBA values, fully gray
                    let color_image = egui::ColorImage::from_rgba_unmultiplied([10, 10], &gray_image);
                    mon.texture = Some(ctx.load_texture("gray_texture", color_image, egui::TextureOptions::default()));
                    ui.painter().rect_filled(monitor_rect, 0.0, egui::Color32::from_gray(50));
                    }
            }
        }
        
        


        
        // Dynamically set the cursor position to the bottom of the bounding box
        let new_ui_position = egui::Pos2::new(bounding_box.min.x, bounding_box.max.y + 10.0);
        ui.allocate_ui_at_rect(
            egui::Rect::from_min_size(new_ui_position, egui::vec2(500.0, 30.0)), 
            |ui| {
                ui.separator(); // Add the separator at the specified position
            }
        );
    }
    
    
    fn draw_monitor_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(selected_idx) = self.selected_monitor {
            let monitor = &mut self.monitors[selected_idx];
    
            // Create a separate box for monitor settings
            ui.group(|ui| {
                ui.label(format!("Monitor {} Settings", monitor.id));
    
                // Enable/disable monitor checkbox
                ui.checkbox(&mut monitor.proposed_status, "Enabled");
    
                // Input field for the proposed resolution
                let mut resolution_input = if let Some((width, height)) = monitor.proposed_resolution {
                    format!("{}x{}", width, height)
                } else {
                    format!("{}x{}", monitor.resolution.0, monitor.resolution.1)
                };
                ui.text_edit_singleline(&mut resolution_input);
    
                // Parse and update proposed resolution if valid
                if let Some((width, height)) = parse_resolution_input(&resolution_input) {
                    monitor.proposed_resolution = Some((width, height));
                }
            });
        }
    
        // Check if any monitor settings have changed
        let mut was_change = false;
        for monitor in &mut self.monitors {
            if monitor.position != monitor.initial_scaled_position || !monitor.enabled {
                was_change = true;
                break;
            }
            if let Some((width, height)) = monitor.proposed_resolution {
                if (width != monitor.resolution.0 || height != monitor.resolution.1) {
                    was_change = true;
                    break;
                }
            }
            if monitor.proposed_status != monitor.enabled{
                was_change = true;
                break;
            }
        }
    
        if was_change && ui.button("Apply").clicked() {
            let mut command = String::from("xrandr");
    
            // Loop through monitors and add their settings to the command only if enabled
            for monitor in &mut self.monitors {
                if monitor.proposed_status == true {
                    let position_x = if monitor.position != monitor.initial_scaled_position {
                        (self.net_zero_x - monitor.position.x) as i32
                    } else {
                        monitor.position.x as i32
                    };
                    let position_y = if monitor.position != monitor.initial_scaled_position {
                        (self.net_zero_y - monitor.position.y) as i32
                    } else {
                        monitor.position.y as i32
                    };
    
                    if let Some((width, height)) = monitor.proposed_resolution {
                        command.push_str(&format!(
                            " --output {} --mode {}x{} --pos {}x{}",
                            monitor.id, width, height, position_x.abs(), position_y
                        ));
                    }
                    monitor.enabled = true; // Set mon to enabled so next time it loops it doesn't break
                } else {
                    // If monitor is disabled, add command to turn it off
                    command.push_str(&format!(" --output {} --off", monitor.id));
                    monitor.enabled = false;  // disable the monitor in its settings
                }
            }
    
            // Execute the combined xrandr command for all monitors
            let _output = std::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .output()
                .expect("Failed to apply monitor settings");
            //dbg!(command);
        }
    }
    
}


fn parse_resolution_input(input: &str) -> Option<(u32, u32)> {
    let parts: Vec<&str> = input.split('x').collect();
    if parts.len() == 2 {
        if let (Ok(width), Ok(height)) = (parts[0].parse(), parts[1].parse()) {
            return Some((width, height));
        }
    }
    None
}

impl App for MonitorApp {
    
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| { 
            self.draw_monitors(ui, ctx);   // Draw monitors on the pane. Can probably draw the rest here.
            // Seperator added at the end of draw monitors
            // Draw monitor settings in a different section of the UI
            self.draw_monitor_settings(ui, ctx);
        });
    }
}


fn get_monitors_from_xrandr() -> Vec<Monitor> {
    let output = Command::new("xrandr")
        .output()
        .expect("Failed to execute xrandr");

    let output_str = str::from_utf8(&output.stdout).expect("Failed to parse xrandr output");
    let mut monitors = Vec::new();

    for line in output_str.lines() {
        if line.contains(" connected") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let id = parts[0];

            // Look for resolution and position in the line
            if let Some(resolution_position) = parts.iter().find(|&s| s.contains('+')) {
                // Extract resolution and position
                let resolution_parts: Vec<&str> = resolution_position.split('x').collect();
                let width = resolution_parts[0].parse::<u32>().unwrap_or(1920);
                let height_position: Vec<&str> = resolution_parts[1].split('+').collect();
                let height = height_position[0].parse::<u32>().unwrap_or(1080);
                let pos_x = height_position[1].parse::<f32>().unwrap_or(0.0);
                let pos_y = height_position[2].parse::<f32>().unwrap_or(0.0);

                monitors.push(Monitor {
                    id: id.to_string(),
                    enabled: true,
                    orientation: String::from("Landscape"),
                    resolution: (width, height),
                    proposed_resolution: Some((width, height)),
                    position: egui::Pos2::new(pos_x, pos_y),
                    initial_scaled_position: egui::Pos2::new(0.0, 0.0),
                    scale: 1.0,
                    proposed_status: true,
                    screenshot: None,
                    duplicate_of: None,
                    being_dragged: false,
                    last_screenshot_time: Instant::now(),
                    texture: None,
                    scale_x_factor: 0.0,
                    scale_y_factor: 0.0,
                });
            } else {
                // Monitor is connected but lacks resolution and position, mark as blank
                monitors.push(Monitor {
                    id: id.to_string(),
                    enabled: false,
                    orientation: String::from("Landscape"),
                    resolution: (1920, 1080), // Placeholder for blank monitors
                    proposed_resolution: None,
                    position: egui::Pos2::new(0.0, 0.0),
                    initial_scaled_position: egui::Pos2::new(0.0, 0.0),
                    scale: 1.0,
                    proposed_status: false,
                    screenshot: None,
                    duplicate_of: None,
                    being_dragged: false,
                    last_screenshot_time: Instant::now(),
                    texture: None,
                    scale_x_factor: 0.0,
                    scale_y_factor: 0.0,
                });
            }
        }
    }

    monitors
}




/*
impl eframe::App for MonitorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Monitor Manager");

            for (index, monitor) in self.monitors.iter_mut().enumerate() {
                if !monitor.enabled {
                    continue;
                }
            
                let monitor_rect = egui::Rect::from_min_size(
                    monitor.position,
                    egui::vec2(
                        monitor.resolution.0 as f32 / 10.0 * monitor.scale,
                        monitor.resolution.1 as f32 / 10.0 * monitor.scale,
                    ),
                );
            
                let monitor_response = ui.allocate_rect(monitor_rect, egui::Sense::click_and_drag());
            
                if monitor_response.hovered() {
                    ui.painter().rect_filled(monitor_rect, 5.0, egui::Color32::from_rgb(150, 200, 250));
                } else {
                    ui.painter().rect_filled(monitor_rect, 5.0, egui::Color32::from_rgb(100, 150, 250));
                }
            
                ui.painter().rect_stroke(monitor_rect, 5.0, egui::Stroke::new(2.0, egui::Color32::BLACK));
            
                // Add some placeholder content preview in the monitor
                let padding = 10.0;
                let content_rect = monitor_rect.shrink(padding);
            
                // Draw some mock "windows" on the monitor
                let mut window_positions = vec![
                    egui::Rect::from_min_size(content_rect.left_top(), egui::vec2(100.0, 50.0)),
                    egui::Rect::from_min_size(content_rect.center(), egui::vec2(80.0, 40.0)),
                    egui::Rect::from_min_size(content_rect.right_bottom() - egui::vec2(70.0, 35.0), egui::vec2(60.0, 35.0)),
                ];
            
                for window_rect in &window_positions {
                    ui.painter().rect_filled(*window_rect, 2.0, egui::Color32::from_rgb(80, 80, 80)); // window background
                    ui.painter().rect_stroke(*window_rect, 2.0, egui::Stroke::new(1.0, egui::Color32::WHITE)); // window border
                    ui.painter().text(
                        window_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "App",
                        egui::TextStyle::Button.resolve(ui.style()),
                        egui::Color32::WHITE,
                    );
                }
            
                // Display monitor label at the bottom
                ui.painter().text(
                    monitor_rect.center_bottom() + egui::vec2(0.0, -5.0),
                    egui::Align2::CENTER_BOTTOM,
                    format!("Monitor {}: {}x{}", monitor.id, monitor.resolution.0, monitor.resolution.1),
                    egui::TextStyle::Body.resolve(ui.style()),
                    egui::Color32::WHITE,
                );

                if monitor_response.clicked() {
                    self.selected_monitor = Some(index);
                }

                if monitor_response.drag_started() {
                    self.drag_start = Some(index);
                }

                if let Some(dragging_index) = self.drag_start {
                    if dragging_index == index && monitor_response.dragged() {
                        monitor.position += monitor_response.drag_delta();
                    }
                    if monitor_response.drag_released() {
                        self.drag_start = None;
                    }
                }
            }
        });

        if let Some(selected) = self.selected_monitor {
            let other_monitors: Vec<(usize, usize)> = self
                .monitors
                .iter()
                .enumerate()
                .filter(|(idx, _)| *idx != selected)
                .map(|(idx, monitor)| (idx, monitor.id))
                .collect();

            egui::Window::new("Monitor Settings")
                .collapsible(false)
                .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -10.0))
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Monitor Settings");

                    let monitor = &mut self.monitors[selected];

                    ui.horizontal(|ui| {
                        ui.label("Enabled:");
                        ui.checkbox(&mut monitor.enabled, "");
                    });

                    ui.horizontal(|ui| {
                        ui.label("Resolution:");
                        ui.add(egui::DragValue::new(&mut monitor.resolution.0).speed(10).clamp_range(800..=3840));
                        ui.label("x");
                        ui.add(egui::DragValue::new(&mut monitor.resolution.1).speed(10).clamp_range(600..=2160));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Orientation:");
                        egui::ComboBox::from_id_source("orientation_combobox")
                            .selected_text(&monitor.orientation)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut monitor.orientation, "Landscape".to_string(), "Landscape");
                                ui.selectable_value(&mut monitor.orientation, "Portrait".to_string(), "Portrait");
                                ui.selectable_value(&mut monitor.orientation, "Landscape (Flipped)".to_string(), "Landscape (Flipped)");
                                ui.selectable_value(&mut monitor.orientation, "Portrait (Flipped)".to_string(), "Portrait (Flipped)");
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Scale:");
                        ui.add(egui::Slider::new(&mut monitor.scale, 0.5..=2.0).text("Scale"));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Duplicate of:");
                        egui::ComboBox::from_id_source("duplicate_combobox")
                            .selected_text(
                                monitor
                                    .duplicate_of
                                    .map(|id| format!("Monitor {}", id + 1))
                                    .unwrap_or_else(|| "None".to_string()),
                            )
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut monitor.duplicate_of, None, "None".to_string());
                                for (other_index, other_id) in &other_monitors {
                                    ui.selectable_value(&mut monitor.duplicate_of, Some(*other_index), format!("Monitor {}", other_id));
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        if ui.button("Apply").clicked() {
                            // Apply the changes
                        }
                        if ui.button("Close").clicked() {
                            self.selected_monitor = None;
                        }
                    });
                });
        }
    }
}
    */


fn main() -> eframe::Result<()> {
    // So you can run multiple things from main. think the eframe is a loop

    let monitors = get_monitors_from_xrandr();
    // Debug print each monitor
    if monitors.is_empty() { // WOuld need something here
        println!("No monitors found.");
    } else {
        for mon in &monitors{
            //dbg!(&mon.id);
            //dbg!(mon.enabled);
        }
    }
    let monitor_app = MonitorApp {
        monitors,
        ..Default::default()
    };
    // Check if no monitors were found
    

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "Monitor Manager",
        options,
        Box::new(|_cc| {
             Ok(Box::new(monitor_app))
        }),
    )

}

/*
To do:
More display preferences right now its the bare minumum
Ability to save display setups like arandr
Snaping to each monitor like arandr
Position feedback to the user to know if unaligned
Resizing of the whole gui
Detection when monitors change to run itself


Issues:
Some small lag when clicking onto a monitor to move it since not threading. Not too noticable

buggy when new mons get introduced. But semi working with how I want it. 
Unsure if bugs still exist but looking much better somehow.

Sources
https://github.com/emilk/egui
https://github.com/quadrupleslap/scrap/tree/master/examples
https://doc.rust-lang.org/book/ch08-01-vectors.html
*/