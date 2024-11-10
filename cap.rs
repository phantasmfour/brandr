use scrap::{Capturer, Display};
use std::io::ErrorKind;
use std::time::Instant;
pub fn capture_screen(display: Display) -> Option<image::DynamicImage> {// Take a display inputed

    let start = Instant::now();
    let mut capturer = Capturer::new(display).expect("Couldn't begin capture.");
        match capturer.frame() {
            Ok(frame) => {
                let buffer = frame.to_vec();
                let width = capturer.width();
                let height = capturer.height();

                // Convert to an image
                let image = image::ImageBuffer::<image::Bgra<u8>, Vec<u8>>::from_raw(width as u32, height as u32, buffer)
                    .expect("Failed to convert the capture to an image");
                // Make a bga image from the cap frame.
                let duration = start.elapsed();
                //println!("Time elapsed: {:?}", duration);
                return Some(image::DynamicImage::ImageBgra8(image));  // Send it back with optionally being none. 
            }
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                return None;
            }
            Err(e) => {
                eprintln!("Error capturing screen: {:?}", e);
                return None;
            }
        }
}
