import type { CSSProperties, PropsWithChildren, ReactNode } from "react";

type BasicProps = PropsWithChildren<{ style?: CSSProperties; onClick?: () => void; disabled?: boolean; className?: string; "aria-label"?: string; label?: ReactNode; description?: ReactNode; childrenLayout?: "below" | "inline" }>;
export function DialogButton(props: BasicProps) { return <button {...props}/>; }
export function Focusable(props: BasicProps) { return <div {...props}/>; }
export function PanelSection({ children, title }: PropsWithChildren<{ title?: string }>) { return <section>{title && <h2>{title}</h2>}{children}</section>; }
export function PanelSectionRow({ children }: PropsWithChildren) { return <div className="panel-row">{children}</div>; }
export function ButtonItem(props: BasicProps) { return <button className="button-item" {...props}/>; }
export function Field({ children, label, description, childrenLayout }: BasicProps) { return <label className={`field ${childrenLayout === "below" ? "field-below" : ""}`}><span>{label}</span>{description}{children}</label>; }
export function TextField(props: { value?: string; onChange?: (event: { target: { value: string } }) => void; disabled?: boolean; inputMode?: "numeric" }) { return <input {...props}/>; }
export function Toggle(props: { value: boolean; disabled?: boolean; onChange?: (checked: boolean) => void }) { return <input type="checkbox" checked={props.value} disabled={props.disabled} onChange={(event) => props.onChange?.(event.target.checked)}/>; }
export function ConfirmModal() { return null; }
export function showModal() { return undefined; }
export const Navigation = { Navigate: () => undefined, NavigateBack: () => undefined, CloseSideMenus: () => undefined };
export const staticClasses = { Title: "title" };
