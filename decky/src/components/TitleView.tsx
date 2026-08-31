import { DialogButton, Navigation } from "@decky/ui";
import { FaCog } from "react-icons/fa";
import type { JSX } from "react";

/**
* @public
* @desc Quick Access title with an accessible native settings action.
*
* @returns Plugin title.
*/
export function TitleView(): JSX.Element {
  const openSettings = (): void => {
    Navigation.CloseSideMenus();
    Navigation.Navigate("/decky-remote-power/settings");
  };
  return <div style={{ display: "flex", alignItems: "center", width: "100%" }}>
    <div style={{ flexGrow: 1 }}>PCs</div>
    <DialogButton aria-label="PC settings" onClick={openSettings} style={{ width: "42px", minWidth: 0, height: "32px", padding: "7px 10px" }}><FaCog aria-hidden="true"/></DialogButton>
  </div>;
}
