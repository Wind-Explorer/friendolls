import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  containsScheme,
  pastedServerAddress,
  splitServerAddress,
  storedServerAddress,
} from "../src/lib/server-endpoint";

describe("server endpoint fields", () => {
  test("normalizes a pasted secure URL and explicit port", () => {
    assert.deepEqual(pastedServerAddress("https://api.example.com:8443/"), {
      address: "api.example.com",
      connectionType: "https",
      port: "8443",
    });
  });

  test("maps WebSocket schemes to their matching HTTP modes", () => {
    assert.deepEqual(pastedServerAddress("wss://api.example.com"), {
      address: "api.example.com",
      connectionType: "https",
      port: "",
    });
    assert.deepEqual(pastedServerAddress("ws://api.example.com"), {
      address: "api.example.com",
      connectionType: "http",
      port: "",
    });
  });

  test("does not normalize URLs containing unsupported components", () => {
    assert.equal(pastedServerAddress("https://api.example.com/v1/ws"), null);
    assert.equal(pastedServerAddress("https://user@api.example.com"), null);
  });

  test("detects a manually typed scheme without changing the address", () => {
    assert.equal(containsScheme("https://api.example.com"), true);
    assert.equal(containsScheme("api.example.com"), false);
  });

  test("round-trips stored proxy and direct addresses", () => {
    assert.equal(
      storedServerAddress("api.example.com", "https"),
      "https://api.example.com",
    );
    assert.deepEqual(splitServerAddress("https://api.example.com"), {
      address: "api.example.com",
      connectionType: "https",
    });
    assert.equal(
      storedServerAddress("lan.example.com", "direct"),
      "lan.example.com",
    );
  });
});
