// @vitest-environment happy-dom

import { screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { SettingGroup } from "../SettingGroup";
import { SettingRow } from "../SettingRow";
import { renderSettings } from "./fixtures";

describe("SettingGroup", () => {
  it("labels the band with its own heading", () => {
    renderSettings(
      <SettingGroup id="general.incidents" title="Incidents">
        <SettingRow setting="keepIncidents" control={<input />} />
      </SettingGroup>,
    );

    const heading = screen.getByRole("heading", { level: 4, name: "Incidents" });
    expect(screen.getByRole("group", { name: "Incidents" })).toContainElement(heading);
  });

  /* The rule belongs to the group below it, so the panel's first band is not
     divided from the panel edge it already sits against. */
  it("draws its rule only when something precedes it", () => {
    renderSettings(
      <SettingGroup id="general.launch" title="Launch">
        <SettingRow setting="autoRun" control={<input />} />
      </SettingGroup>,
    );

    const group = screen.getByRole("group", { name: "Launch" });
    expect(group).not.toHaveClass("border-t");
    expect(group).toHaveClass("first:pt-0");
    expect(within(group).getByRole("separator")).toHaveClass("group-first/setting-group:hidden");
  });
});
