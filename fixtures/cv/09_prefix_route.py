state = tools.fetch_state(request_id)
if state["filename"].startswith("draft-"):
    print("draft")
else:
    print("final")
