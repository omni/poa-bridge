use std::time::{Duration, Instant};

use reqwest;
use web3::types::U256;

use config::{Config, GasPriceSpeed, Node};

const GAS_PRICE_CACHE_DURATION: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Deserialize)]
struct GasPriceJson {
    block_number: u64,
    block_time: f64,
    health: bool,
    instant: f64,
    fast: f64,
    standard: f64,
    slow: f64
}

impl GasPriceJson {
    fn get_price_for_speed(&self, speed: &GasPriceSpeed) -> f64 {
        match *speed {
            GasPriceSpeed::Instant => self.instant,
            GasPriceSpeed::Fast => self.fast,
            GasPriceSpeed::Standard => self.standard,
            GasPriceSpeed::Slow => self.slow
        }
    }
}

#[derive(Debug)]
struct GasPriceNode {
    client: reqwest::Client,
    url: String,
    speed: GasPriceSpeed,
    default_price: f64,
    cache_timer: Instant,
    cached_price: Option<f64>
}

impl<'a> From<&'a Node> for GasPriceNode {
    fn from(node: &'a Node) -> Self {
        let client = reqwest::Client::builder()
            .timeout(node.gas_price_timeout)
            .build().unwrap();
    
        GasPriceNode {
            client,
            url: node.gas_price_oracle_url.clone(),
            speed: node.gas_price_speed,
            default_price: node.default_gas_price,
            cache_timer: Instant::now(),
            cached_price: None
        }
    }
}

impl GasPriceNode {
    fn cache_has_expired(&self) -> bool {
        self.cache_timer.elapsed() > GAS_PRICE_CACHE_DURATION
    }

    fn get_cached_price(&self) -> Option<f64> {
        match self.cache_has_expired() {
            true => None,
            false => self.cached_price
        }
    }

    fn request_price(&self) -> Result<f64, ()> {
        if let Ok(mut resp) = reqwest::get(&self.url) {
            let des: reqwest::Result<GasPriceJson> = resp.json();
            if let Ok(obj) = des {
                return Ok(obj.get_price_for_speed(&self.speed));
            }
        }
        Err(())
    }

    fn get_price(&mut self) -> U256 {
        let price = if let Some(cached_price) = self.get_cached_price() {
            cached_price
        } else if let Ok(price) = self.request_price() {
            self.cached_price = Some(price);
            self.cache_timer = Instant::now();
            price
        } else {
            self.default_price
        };

        (price.ceil() as u64).into()
    }
}

#[derive(Debug)]
pub struct GasPriceClient {
    home: GasPriceNode,
    foreign: GasPriceNode
}

impl<'a> From<&'a Config> for GasPriceClient {
    fn from(config: &'a Config) -> Self {
        let home = GasPriceNode::from(&config.home);
        let foreign = GasPriceNode::from(&config.foreign);
        GasPriceClient { home, foreign }
    }
}

impl GasPriceClient {
    pub fn get_home_price(&mut self) -> U256 {
        self.home.get_price()
    }

    pub fn get_foreign_price(&mut self) -> U256 {
        self.foreign.get_price()
    }
}

