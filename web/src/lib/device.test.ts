import { expect, test } from "bun:test";
import { deviceName } from "./device";

test("names the browser and system people would", () => {
  expect(
    deviceName(
      "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36 Edg/140.0.0.0",
    ),
  ).toBe("Edge · Windows");
  expect(
    deviceName(
      "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
    ),
  ).toBe("Safari · iOS");
  expect(
    deviceName(
      "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36",
    ),
  ).toBe("Chrome · Android");
  expect(
    deviceName(
      "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5; rv:130.0) Gecko/20100101 Firefox/130.0",
    ),
  ).toBe("Firefox · macOS");
  expect(deviceName("curl/8.0")).toBeNull();
  expect(deviceName(null)).toBeNull();
});
