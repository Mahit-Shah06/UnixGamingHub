fn main() {
    match ugh_core::sensors::sensor_names() {
        Some(sensors) => {
            for s in sensors {
                match s.read() {
                    Some(v) => println!("{}  {:.1} {}", s.device, v, s.unit),
                    None => println!("{} ?", s.device),
                }
            }
        }
        None => println!("couldn't read sensors"),
    }
}
