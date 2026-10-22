import { afterEach, expect, test, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Alerts, Timeline } from "../src/App";
import { api } from "../src/api";
import type { Alert } from "../src/types";
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});
const alert: Alert = {
  id: "a",
  name: "Outbound volume",
  detector: "transfer",
  source: "192.0.2.1",
  destination: null,
  timestamp_ms: 1767225600000,
  window_start_ms: 1767225600000,
  window_end_ms: 1767225660000,
  statistic: "outbound bytes",
  observed: 6000000,
  threshold: 5000000,
  baseline: null,
  supporting_events: 3,
  severity: "high",
  explanation: "Volume crossed threshold.",
  caveat: "This does not confirm exfiltration.",
};
test("alert evidence preserves threshold and caveat", () => {
  render(<Alerts alerts={[alert]} />);
  fireEvent.click(screen.getByRole("button", { name: /Outbound volume/ }));
  expect(screen.getByLabelText("Alert evidence").textContent).toContain(
    "does not confirm exfiltration",
  );
  expect(screen.getByLabelText("Alert evidence").textContent).toContain(
    "6,000,000 / 5,000,000",
  );
});
test("detector filter handles no matches", () => {
  render(<Alerts alerts={[alert]} />);
  fireEvent.change(screen.getByLabelText("Filter alerts"), {
    target: { value: "scan" },
  });
  expect(screen.getByText(/No matching alerts/)).toBeTruthy();
});
test("timeline empty state is explicit", () => {
  render(<Timeline buckets={[]} />);
  expect(screen.getByText(/No observations yet/)).toBeTruthy();
});
test("timeline exposes accessible controls and units", () => {
  render(
    <Timeline
      buckets={[{ start_ms: 1767225600000, events: 3, bytes: 100, alerts: 1 }]}
    />,
  );
  expect(
    screen.getByRole("button", { name: "00:00:00 UTC: 3 events" }),
  ).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "bytes" }));
  expect(
    screen.getByRole("button", { name: "00:00:00 UTC: 100 bytes" }),
  ).toBeTruthy();
});
test("API sends actual values and mutation header", async () => {
  const fetch = vi
    .fn()
    .mockResolvedValue({ ok: true, json: async () => ({ started: true }) });
  vi.stubGlobal("fetch", fetch);
  await api("/api/replay", { seed: 42 });
  expect(fetch.mock.calls[0][1].headers["X-SentinelFlow"]).toBe("1");
  expect(JSON.parse(fetch.mock.calls[0][1].body).seed).toBe(42);
});
test("API error is surfaced", async () => {
  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: false,
      status: 409,
      text: async () => "Replay already running",
    }),
  );
  await expect(api("/api/replay", {})).rejects.toThrow(
    "409: Replay already running",
  );
});
