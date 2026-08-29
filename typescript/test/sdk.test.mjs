import test from "node:test";
import assert from "node:assert/strict";
import { ReflexClient } from "../dist/index.js";

test("ReflexClient instantiation and models", () => {
  const client = new ReflexClient("http://127.0.0.1:9199");
  assert.ok(client);
});
