state = tools.fetch_state(request_id)
if state["category"] == "incident":
    receipt = tools.emit_text("alerts/" + state["filename"])
else:
    receipt = tools.emit_text("archive/" + state["filename"])
print(receipt)
