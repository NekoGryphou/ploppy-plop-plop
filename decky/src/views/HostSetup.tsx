import { ButtonItem, Navigation, PanelSection, PanelSectionRow } from "@decky/ui";
import type { JSX } from "react";

/**
* @public
* @desc Host setup instructions shown only during administration.
*
* @returns Setup guidance.
*/
export function HostSetup(): JSX.Element {
  return <PanelSection title="DeckyMyRigHost setup">
    <PanelSectionRow><div>
      <ol>
        <li>Download and install DeckyMyRigHost on the Windows PC. Administrator permission is needed to register its service and Private-network firewall rule.</li>
        <li>The service starts automatically. No Python, Node.js, Java, SSH, SSH keys, or Windows password is needed.</li>
        <li>The default port is 47991. Change <code>DeckyMyRigHost.toml</code>, synchronize the firewall rule, restart the service, and enter the same port in Decky when using a custom value.</li>
        <li>Add and save the PC without a pairing code. When the PC is awake, use its separate Pair action and enter the temporary six-digit code.</li>
      </ol>
    </div></PanelSectionRow>
    <PanelSectionRow><ButtonItem layout="below" onClick={() => Navigation.NavigateBack()}>Back</ButtonItem></PanelSectionRow>
  </PanelSection>;
}
