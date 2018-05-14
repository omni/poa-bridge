use std::time::{Duration, Instant};

use reqwest;
use web3::types::U256;

use config::{Config, GasPriceSpeed, Node};

// The duration for which a gas price is valid once it has been received
// from a gas price oracle URL.
const GAS_PRICE_CACHE_DURATION: Duration = Duration::from_secs(5 * 60);

/// Represents the JSON body of an HTTP response received from a POA gas
/// price oracle.
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

/// Contains the data necessary to query either the home or foreign gas
/// price oracle.
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
    // Checks whether or not that the time that the data stored in
    // `self.cached_price` has exceeded the cache time.
    fn cache_has_expired(&self) -> bool {
        self.cache_timer.elapsed() > GAS_PRICE_CACHE_DURATION
    }

    // Returns None if the cached price has expired or was never set,
    // otherwise returns the cached price.
    fn get_cached_price(&self) -> Option<f64> {
        match self.cache_has_expired() {
            true => None,
            false => self.cached_price
        }
    }

    // Makes an HTTP request to the oracle URL, get's the value for the
    // JSON key corresponding to `self.speed`.
    fn request_price(&self) -> Result<f64, ()> {
        if let Ok(mut resp) = reqwest::get(&self.url) {
            let des: reqwest::Result<GasPriceJson> = resp.json();
            if let Ok(obj) = des {
                return Ok(obj.get_price_for_speed(&self.speed));
            }
        }
        Err(())
    }

    // Returns the cached price if the cache is set and has not yet
    // expired, returns the price from the oracle URL if the cache is
    // empty or expired, returns the defualt price if an HTTP networking or
    // JSON deserializtion errors (malformed JSON response) occurs.
    //
    // This method returns a U256 as that is the required type for the
    // `gas_price` field in `ethcore_transaction::Transaction`.
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

/// Holds the data required to get the gas price for the home and foreign
/// nodes.
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

