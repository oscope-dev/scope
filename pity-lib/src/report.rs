use std::fs::File;
use std::io::Write;

pub fn write_to_report_file(prefix: &str, text: &str) -> anyhow::Result<String> {
    let id = nanoid::nanoid!(10, &nanoid::alphabet::SAFE);

    let file_path = format!("/tmp/pity/pity-{}-{}.txt", prefix, id);
    let mut file = File::create(&file_path)?;
    file.write_all(text.as_bytes())?;

    Ok(file_path)
}