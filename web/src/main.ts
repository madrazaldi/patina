import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

if (navigator.userAgent.includes("Mac")) document.documentElement.classList.add("mac");

// The native context menu (Reload, Inspect…) only makes sense while developing.
if (!import.meta.env.DEV) document.addEventListener("contextmenu", (e) => e.preventDefault());

mount(App, { target: document.getElementById("app")! });
