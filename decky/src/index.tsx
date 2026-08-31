import { definePlugin, routerHook } from "@decky/api";
import { FaPowerOff } from "react-icons/fa";
import { TitleView } from "./components/TitleView";
import { QuickAccess } from "./views/QuickAccess";
import { Settings } from "./views/Settings";
import { HostSetup } from "./views/HostSetup";

export default definePlugin(() => {
  routerHook.addRoute("/decky-remote-power/settings", Settings);
  routerHook.addRoute("/decky-remote-power/host-setup", HostSetup);
  return {
    name: "Decky My Rig",
    titleView: <TitleView/>,
    content: <QuickAccess/>,
    icon: <FaPowerOff/>,
    onDismount(): void {
      routerHook.removeRoute("/decky-remote-power/settings");
      routerHook.removeRoute("/decky-remote-power/host-setup");
    }
  };
});
