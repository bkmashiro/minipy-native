async def main(batch_id: i64, price_limit: i64) -> i64:
    quotes = await tools.load_quotes(batch_id)
    total: i64 = 0
    for q in quotes:
        if q.quantity > 0 and q.unit_price_cents <= price_limit:
            total += q.unit_price_cents * q.quantity
    return await tools.record_total(batch_id, total)
