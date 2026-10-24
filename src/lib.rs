//! Generation is independent of the UI. Secrets never implement Debug or Serialize.
pub mod clipboard;
pub mod generator;
pub mod system;
pub mod ui;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const TELEGRAM_URL: &str = "https://t.me/rercon";
pub const TELEGRAM_HANDLE: &str = "t.me/rercon";
/// Larry 3D, the same FIGlet face as SNATCH and RAZE. Bundled as plain ASCII.
pub const WORDMARK: &str = r#" ____    __       __    __  ____    __  __
/\  _`\ /\ \     /\ \  /\ \/\  _`\ /\ \/\ \
\ \ \L\_\ \ \    \ `\`\\/'/\ \ \L\ \ \ \_\ \
 \ \ \L_L\ \ \  __`\ `\ /'  \ \ ,__/\ \  _  \
  \ \ \/, \ \ \L\ \ `\ \ \   \ \ \/  \ \ \ \ \
   \ \____/\ \____/   \ \_\   \ \_\   \ \_\ \_\
    \/___/  \/___/     \/_/    \/_/    \/_/\/_/"#;
