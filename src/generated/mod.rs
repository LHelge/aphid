mod favicon;
mod feed;
mod robots;
mod sitemap;

pub use favicon::FaviconSet;
pub use feed::{AtomFeed, RssFeed};
pub use robots::Robots;
pub use sitemap::Sitemap;
