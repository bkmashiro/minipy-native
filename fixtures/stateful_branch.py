async def main(batch_id: i64, price_limit: i64) -> i64:
    quotes = await tools.load_quotes(batch_id)
    total: i64 = 0
    previous_price: i64 = 0
    descending: bool = False
    for q in quotes:
        if q.quantity > 0 and q.unit_price_cents <= price_limit and q.unit_price_cents >= previous_price:
            total += q.unit_price_cents * q.quantity
            previous_price = q.unit_price_cents
        else:
            descending = True
            total -= 0
    return await tools.record_total(batch_id, total)
