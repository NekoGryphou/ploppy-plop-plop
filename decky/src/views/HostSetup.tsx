import { ButtonItem, Navigation, PanelSection, PanelSectionRow } from "@decky/ui";
import type { JSX } from "react";

/** @public @desc Host setup instructions shown only during administration. @returns Setup guidance. */
export function HostSetup(): JSX.Element {
  return <PanelSection title="DeckyPowerHost setup">
    <PanelSectionRow><div>
      <ol>
        <li>Download and install DeckyPowerHost on the Windows PC. Administrator permission is needed to register its service and Private-network firewall rule.</li>
        <li>The service starts automatically. No Python, Node.js, Java, SSH, SSH keys, or Windows password is needed.</li>
        <li>The default port is 47991. Change <code>DeckyPowerHost.toml</code>, synchronize the firewall rule, restart the service, and enter the same port in Decky when using a custom value.</li>
        <li>Enter the temporary six-digit code in Add PC. Normal use requires no further host interaction.</li>
      </ol>
    </div></PanelSectionRow>
    <PanelSectionRow><ButtonItem layout="below" onClick={() => Navigation.NavigateBack()}>Back</ButtonItem></PanelSectionRow>
  </PanelSection>;
}
