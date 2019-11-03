use crate::message::OpenMarketState;
use crate::MarketData;


pub fn compute_payment(market_data: MarketData, prev_market_data: MarketData, position_size: i64) -> i64 {
    let decimal_precision = 100000000i64;
    let change_in_price = market_data.bitcoin.usd - prev_market_data.bitcoin.usd; // change in USD
    let percent_change_in_price = change_in_price * decimal_precision / prev_market_data.bitcoin.usd ;
    let profit_or_loss = position_size * percent_change_in_price / decimal_precision;
    println!("PAyment is {}", profit_or_loss);
    profit_or_loss
}