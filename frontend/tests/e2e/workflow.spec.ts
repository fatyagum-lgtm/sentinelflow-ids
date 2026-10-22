import { test, expect } from "@playwright/test";
import { mkdir } from "node:fs/promises";
test("actual replay, evidence, research, export and mobile layout", async ({
  page,
  request,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(e.message));
  await mkdir("../docs/screenshots", { recursive: true });
  await page.goto("/");
  await expect(
    page.getByRole("button", { name: /Start replay/ }),
  ).toBeEnabled();
  await page
    .getByRole("combobox", { name: "Replay speed", exact: true })
    .selectOption("20");
  await page.getByRole("button", { name: /Start replay/ }).click();
  await expect(page.getByRole("button", { name: /Stop replay/ })).toBeVisible();
  await page
    .getByRole("navigation")
    .getByRole("button", { name: /Replay/ })
    .click();
  await expect
    .poll(async () =>
      Number(
        (await page.getByTestId("accepted-events").textContent())?.replaceAll(
          ",",
          "",
        ),
      ),
    )
    .toBeGreaterThan(50);
  await page.screenshot({
    path: "../docs/screenshots/replay.png",
    fullPage: true,
  });
  await expect(page.getByRole("button", { name: /Start replay/ })).toBeVisible({
    timeout: 20000,
  });
  const v = await (await request.get("/api/status")).json();
  expect(v.phase).toBe("complete");
  expect(v.snapshot.metrics.accepted).toBe(822);
  for (const id of ["scan", "rate", "transfer", "beacon"])
    expect(v.snapshot.metrics.by_detector[id]).toBeGreaterThan(0);
  await expect(page.getByTestId("accepted-events")).toHaveText("822");
  await page.getByRole("button", { name: /Overview/ }).click();
  await page.screenshot({
    path: "../docs/screenshots/overview.png",
    fullPage: true,
  });
  await page.getByLabel("Filter alerts").selectOption("transfer");
  await page.getByRole("button", { name: /Outbound volume/ }).click();
  await expect(page.getByLabel("Alert evidence")).toContainText(
    "does not confirm exfiltration",
  );
  const download = page.waitForEvent("download");
  await page.getByRole("link", { name: /Export JSON/ }).click();
  expect((await download).suggestedFilename()).toBe("sentinelflow-report.json");
  await page.getByRole("button", { name: /Detection lab/ }).click();
  await page.screenshot({
    path: "../docs/screenshots/detectors.png",
    fullPage: true,
  });
  await page.getByRole("button", { name: /Research/ }).click();
  await expect(
    page.getByRole("heading", { name: "Measured throughput" }),
  ).toBeVisible();
  await page.screenshot({
    path: "../docs/screenshots/research.png",
    fullPage: true,
  });
  await page.setViewportSize({ width: 390, height: 844 });
  await page.getByRole("button", { name: /Overview/ }).click();
  await page.screenshot({
    path: "../docs/screenshots/mobile.png",
    fullPage: true,
  });
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBeTruthy();
  expect(errors).toEqual([]);
});
test("invalid seed and cancellation", async ({ page, request }) => {
  await page.goto("/");
  await expect(
    page.getByRole("button", { name: /Start replay/ }),
  ).toBeEnabled();
  await page.getByLabel("Random seed").fill("-1");
  await page.getByRole("button", { name: /Start replay/ }).click();
  await expect(page.getByRole("alert")).toContainText(
    "nonnegative safe integer",
  );
  await page.getByLabel("Random seed").fill("7");
  await page
    .getByRole("combobox", { name: "Replay speed", exact: true })
    .selectOption("20");
  await page.getByRole("button", { name: /Start replay/ }).click();
  await page.getByRole("button", { name: /Stop replay/ }).click();
  await expect
    .poll(async () => (await (await request.get("/api/status")).json()).phase)
    .toBe("stopped");
  expect(
    (
      await request.post("/api/replay", { data: { path: "/etc/passwd" } })
    ).status(),
  ).toBe(403);
});
