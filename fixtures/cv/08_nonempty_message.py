state = tools.fetch_state(request_id)
if state["message"] != "":
    receipt = tools.emit_text(state["message"])
    print(receipt)
else:
    print("no message")
