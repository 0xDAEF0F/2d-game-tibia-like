use env_logger::{Builder, Env};
use std::io::Write;

pub struct MmoLogger;

impl MmoLogger {
	pub fn init(log_level: &'static str) {
		let mut builder = init_builder();

		let log_level = match log_level {
			"error" => log::LevelFilter::Error,
			"warn" => log::LevelFilter::Warn,
			"info" => log::LevelFilter::Info,
			"debug" => log::LevelFilter::Debug,
			"trace" => log::LevelFilter::Trace,
			_ => log::LevelFilter::Info,
		};

		builder.filter(None, log_level);

		builder.init();
	}
}

fn init_builder() -> Builder {
	let mut builder = Builder::from_env(Env::default().default_filter_or("info"));

	builder.format(|buf, record| {
		let timestamp = chrono::Local::now().format("%H:%M:%S").to_string();

		let target = record.target();
		let target_parts: Vec<&str> = target.split("::").collect();
		let short_target = target_parts.last().unwrap_or(&target);

		let level_color = match record.level() {
			log::Level::Error => "\x1b[31m", // Red
			log::Level::Warn => "\x1b[33m",  // Yellow
			log::Level::Info => "\x1b[32m",  // Green
			log::Level::Debug => "\x1b[36m", // Cyan
			log::Level::Trace => "\x1b[35m", // Magenta
		};
		let reset_color = "\x1b[0m";

		writeln!(
			buf,
			"[{} {}{}{} {}] {}",
			timestamp,
			level_color,
			record.level(),
			reset_color,
			short_target,
			record.args()
		)
	});

	builder
}
