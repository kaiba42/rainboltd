use crate::message::OpenMarketState;

pub fn compute_payment(market: &mut OpenMarketState, current_index_price: f64, position_size: i64) -> i64 {
    let change_in_price = current_index_price - market.last_index_price;
    let profit_or_loss = (position_size as f64) * change_in_price / market.last_index_price;
    market.last_index_price = current_index_price;
    profit_or_loss as i64
}