import unittest
import os

import toml
import requests

class TestUsers(unittest.TestCase):
    def __init__(self, *args, **kwargs):
        super(TestUsers, self).__init__(*args, **kwargs)
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

    def test_user_settings(self):
        """
        Test updating user settings
        """
        r = requests.put(f"{self.base}/user_settings", json={ "settings": {"dark_mode": 1}}, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Failed to set user settings")

        r = requests.get(f"{self.base}/user_settings", cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Failed to get user settings")
        self.assertEqual(r.json()["dark_mode"], 1, "Settings failed to update or not JSON")

    def test_user_search(self):
        """
        Test searching for users as well as
        getting information about a user
        """
        r = requests.get(f"{self.base}/users/search", { "filter": self.user[:3] }, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Failed to find users")
        self.assertIn(self.user, [u["id"] for u in r.json()["users"]], "Search did not find self")

        r = requests.get(f"{self.base}/users", { "id": self.user }, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Failed to find self")
        self.assertEqual(r.json()["id"], self.user)

if __name__ == '__main__':
    unittest.main()
