import unittest
import os

import toml
import requests

class TestLogin(unittest.TestCase):
    def __init__(self, *args, **kwargs):
        super(TestLogin, self).__init__(*args, **kwargs)
        self.user = os.getenv("TEST_API_USER")
        self.password = os.getenv("TEST_API_PASSWORD")
        with open("../config.toml", "r") as f:
            self.config = toml.loads(f.read())
            self.port = self.config["server"]["port"]
            self.base = f"http://localhost:{self.port}/v1"
            self.cookies = requests.cookies.RequestsCookieJar()

    def test_login(self):
        """
        Ensure user can login, username and password should be stored in the
        TEST_API_USER and TEST_API_PASSWORD env variables respectively
        """
        r = requests.post(f"{self.base}/login", {"username": self.user, "password": self.password})
        self.assertEqual(r.status_code, 200, "Login failure (Did you set env TEST_API_USER and TEST_API_PASSWORD?)")
        self.cookies.update(r.cookies)

        r = requests.post(f"{self.base}/login", {"username": self.user, "password": self.password + "not_valid"})
        self.assertEqual(r.status_code, 401, "Login doesn't validate password properly")

        r = requests.post(f"{self.base}/logout")
        self.assertTrue(len(r.cookies) == 0, "Logged out successfully")

if __name__ == '__main__':
    unittest.main()
