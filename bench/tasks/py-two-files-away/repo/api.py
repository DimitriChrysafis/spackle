import db

def get_user(user_id):
    row = db.row_for(user_id)
    if row is None:
        return {"error": "not found"}
    return row
