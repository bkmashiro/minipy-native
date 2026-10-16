state = tools.fetch_state(request_id)
message = state["message"]
if "error" in message:
    print("needs attention: " + message)
else:
    print("clear")
