fn main() {
   match ugh_core::hwmon::find_sensors() {
       Some(sensors) => {
           loop{
               print!("\x1B[H\x1B[2J\x1B[3J");
               let mut last = String::new();
               for s in &sensors {
                   if last != s.device {
                       println!("{}", s.device);
                       last = s.device.clone();
                   }
                   match s.read() {
                       Some(v) => println!( "  {:<14} {:.1} {}", s.label, v, s.unit),
                       None => println!("{} ?", s.label),
                   }
               }
               println!("Press Ctrl+C to exit");
               std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
        None => println!("couldn't read sensors"),
    }
}
