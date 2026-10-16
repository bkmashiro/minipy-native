stats = tools.fetch_stats(request_id)
if stats["used"] < stats["limit"]:
    ack = tools.publish(request_id, stats["used"] + 1)
else:
    ack = tools.publish(request_id, stats["limit"])
print(ack)
