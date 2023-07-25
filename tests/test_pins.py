import unittest
import os

import toml
import requests

class TestPins(unittest.TestCase):
    def __init__(self, *args, **kwargs):
        super(TestPins, self).__init__(*args, **kwargs)
        self.user = os.getenv("TEST_API_USER")
        self.password = os.getenv("TEST_API_PASSWORD")
        with open("../config.toml", "r") as f:
            self.config = toml.loads(f.read())
            self.port = self.config["server"]["port"]
            self.base = f"http://localhost:{self.port}/v1"
            self.cookies = requests.cookies.RequestsCookieJar()

        r = requests.post(f"{self.base}/login", {"username": self.user, "password": self.password})
        self.assertEqual(r.status_code, 200, "Login failure (Did you set env TEST_API_USER and TEST_API_PASSWORD?)")
        self.cookies.update(r.cookies)

    def test_simple_update_pins(self):
        """
        Test creating, editing, and deleting pins
        """
        params = {
            "name": "My Board",
            "desc": "My board description",
            "color": "#FF0000",
            "perms": {}
        }
        r = requests.post(f"{self.base}/board/boards", json=params, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful board creation")
        id = r.json()["id"]

        # Create pin
        params = {
            "pin_type": 0,
            "flags": 0,
            "board_id": id,
            "content": "Hello world",
            "attachment_paths": [],
            "metadata": {}
        }
        r = requests.post(f"{self.base}/board/pins", json=params, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful pin creation")
        pin_id = r.json()["id"]

        # Edit pin
        params = {
            "id": pin_id,
            "flags": "LOCKED | ARCHIVED",
            "content": "Goodbye world"
        }
        r = requests.put(f"{self.base}/board/pins", json=params, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful pin edit")

        # Get pin and test
        r = requests.get(f"{self.base}/board/pins/single", { "id": pin_id }, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful get pin")
        self.assertEqual(r.json()["flags"], "LOCKED | ARCHIVED", "Successful pin edit")

        # Search pins
        r = requests.get(f"{self.base}/board/pins", { "limit": 1, "query": "WoRlD" }, cookies=self.cookies)
        self.assertEqual(len(r.json()["pins"]), 1, "A pin was returned")

        # Delete pin
        r = requests.delete(f"{self.base}/board/pins", json={ "id": pin_id }, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful pin deletion")

        # Delete parent board
        r = requests.delete(f"{self.base}/board/boards", json={ "id": id }, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful board deletion")

if __name__ == '__main__':
    unittest.main()
