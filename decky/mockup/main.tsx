import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { FaChevronLeft, FaPowerOff } from "react-icons/fa";
import { DeviceRow } from "../src/components/DeviceRow";
import { DeviceForm } from "../src/components/DeviceForm";
import { TitleView } from "../src/components/TitleView";
import { mockDevices } from "./fixtures";
import "./style.css";

function Showcase() {
    return <main>
        <div className="deck">
            <header><FaPowerOff className="power" aria-hidden="true"/>
                <div className="mock-title"><TitleView/></div>
            </header>
            <div className="content">
                <DeviceRow device={mockDevices[0]} status={{state: "online", pairing: "paired", message: "Online"}}
                           onAction={() => undefined}/>
                <DeviceRow device={mockDevices[1]} status={{state: "offline", pairing: "unpaired", message: "Offline"}}
                           onAction={() => undefined}/>
                <DeviceRow device={mockDevices[2]} status={{state: "starting", pairing: "paired", message: "Starting"}}
                           onAction={() => undefined}/>
            </div>
        </div>
        <div className="deck settings">
            <header><button aria-label="Back to Remote PCs" className="back"><FaChevronLeft aria-hidden="true"/></button><h1>Add PC</h1></header>
            <div className="content"><DeviceForm onSaved={() => undefined} onCancel={() => undefined}/></div>
        </div>
    </main>;
}

const root = document.getElementById("root");
if (!root) throw new Error("Mockup root element is missing.");
createRoot(root).render(<StrictMode><Showcase/></StrictMode>);
