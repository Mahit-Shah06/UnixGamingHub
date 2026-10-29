fn main() {
    match ugh_core::sensors::sensor_names() {
        Some(sensors) => {
            let mut last = String::new();
            for s in sensors {
                if last != s.device {
                    println!("{}", s.device);
                    last = s.device.clone();
                }
                match s.read() {
                    Some(v) => println!( "  {:<14} {:.1} {}", s.label, v, s.unit),
                    None => println!("{} ?", s.device),
                }
            }
        }
        None => println!("couldn't read sensors"),
    }
}
