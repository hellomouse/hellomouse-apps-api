import unittest
import os

import toml
import requests

class TestBoards(unittest.TestCase):
    def __init__(self, *args, **kwargs):
        super(TestBoards, self).__init__(*args, **kwargs)
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

    def test_simple_update_boards(self):
        """
        Test creating, editing, and deleting boards
        """
        params = {
            "name": "My Board",
            "desc": "My board description",
            "color": "#FF0000",
            "perms": {}
        }

        r = requests.post(f"{self.base}/board/boards",
            json=params,
            cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful board creation")
        id = r.json()["id"]

        # Test: invalid color
        params["color"] = "not a color"
        r = requests.post(f"{self.base}/board/boards", json=params, cookies=self.cookies)
        self.assertNotEqual(r.status_code, 200, "Invalid color")

        # Test: invalid perms
        params["color"] = "#FF0000"
        params["perms"] = { "not a real user": { "perm_level": "Owner" }}
        r = requests.post(f"{self.base}/board/boards", json=params, cookies=self.cookies)
        self.assertNotEqual(r.status_code, 200, "Invalid perms")

        # Test edit
        params = {
            "id": id,
            "name": "My Board Updated 2",
            "desc": "My board description 2"
        }

        r = requests.put(f"{self.base}/board/boards", json=params, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful board edit")

        # Edit board with null perms: should not remove owner permission
        params = {
            "id": id,
            "name": "My Board Updated 3",
            "desc": "My board description 3",
            "perms": {}
        }
        r = requests.put(f"{self.base}/board/boards", json=params, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Board edit with no perms -> owner perm should exist")

        r = requests.get(f"{self.base}/board/boards/single", { "id": id }, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Get board result")
        self.assertIn(self.user, r.json()["perms"].keys(), "Owner permission not deleted")

        # Add new permission for self
        params["perms"][self.user] = { "perm_level": "Edit" }
        r = requests.put(f"{self.base}/board/boards", json=params, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Board edit self reduce perms -> no effect")
        r = requests.get(f"{self.base}/board/boards/single", { "id": id }, cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Get board result")
        self.assertIn(self.user, r.json()["perms"].keys(), "Owner permission not deleted")

        # Test board search
        r = requests.get(f"{self.base}/board/boards", { "limit": 1, "query": "3" }, cookies=self.cookies)
        self.assertEqual(len(r.json()["boards"]), 1, "A board was returned")

        # Test board delete
        r = requests.delete(f"{self.base}/board/boards",
            json={ "id": id },
            cookies=self.cookies)
        self.assertEqual(r.status_code, 200, "Successful board deletion")

if __name__ == '__main__':
    unittest.main()
