// document.getElementById('content').innerHTML =
// marked.parse('# Marked in the browser\n\nRendered by **marked**.');

import AppEnvironment from "../../types/AppEnvironment";
import "./index.scss";
import init, { run } from "silt-lua";

const EXAMPLE = `-- Standard Lua applies, for the most part. See https://github.com/Auxnon/silt-lua for updates.
-- For best performance use local scope. Stack based VM written in rust.
-- Many compile time errors are not caught yet. Standard library and meta functions not yet ready.
do
    local a = 1
    local b = 2
    print("Sum of " .. a .. " + " .. b .. " = " .. a + b)
    print("Int or float types " .. 1 + 2.5 / 3)
    print("String inference works, '2'+2=" .. "2" + 2)
    local function closure()
        local c = 0
        local function nested()
            c = c + a
            return "Closures work " .. c
        end
        return nested
    end
    local iterate = closure()
    print(iterate())
    print(iterate())
    print("You can also return values to the console.")
    return "Completed!"
end`;

export default class Code extends AppEnvironment {
  panel: HTMLElement;
  area: HTMLTextAreaElement;
  back: HTMLElement;
  output: HTMLElement;
  lines: number = 0;
  domLines: number = 0;
  runner?: (s: string) => string;
  throttle;
  constructor(private dom: HTMLElement, id: number) {
    super(dom, id);
    this.resolver();
    init().then(() => {
      // window.go = (s) => run(s);
      this.runner = (s) => run(s);
      // @ts-ignore
      window.jprintln = (s) => this.println(s);
      // let a = run("1+2");
      // alert(a);
    });
    this.makePanel();
    this.refreshCode();
  }

  open(canvas?: HTMLElement): void {
    if (this.dom.children.length > 1) {
      if (!this.dom.querySelector(".code-panel")) {
        this.dom.appendChild(this.panel);
      }
      if (!this.dom.querySelector(".backpanel")) {
        const extra = document.createElement("div");
        extra.classList.add("code-panel", "backpanel");
        this.dom.insertBefore(extra, this.panel);
      }

      return;
    }
    this.dom.appendChild(this.panel);
  }

  refreshCode() {
    const fontSize = 32; //parseFloat(getComputedStyle(this.area).fontSize) || 10;
    const slices = this.area.value.split("\n");
    this.lines = slices.length;
    if (this.lines > this.domLines) {
      for (let i = this.domLines; i < this.lines; i++) {
        this.addLine();
      }
    } else if (this.lines < this.domLines) {
      for (let i = this.domLines; i > this.lines; i--) {
        this.back.removeChild(this.back.lastChild as HTMLElement);
      }
      this.domLines = this.lines;
    }
    // for (let i = 0; i < this.lines; i++) {
    //   (this.back.childNodes[i] as HTMLElement).style.width =
    //     slices[i].length + "rem";
    // }
    this.area.style.height = this.lines * fontSize + "px";
  }

  makePanel() {
    const p = document.createElement("div");
    p.classList.add("code-panel");

    const seg = document.createElement("div");
    seg.classList.add("code-segment");
    p.appendChild(seg);

    const a = document.createElement("textarea");
    a.classList.add("code-area");
    a.value = EXAMPLE;
    this.area = a;
    this.area.addEventListener("keydown", (ev: KeyboardEvent) => {
      this.keycheck(ev);
    });
    seg.appendChild(a);

    const b = document.createElement("div");
    b.classList.add("code-back");
    seg.appendChild(b);
    this.back = b;

    const button = document.createElement("button");
    button.classList.add("code-run");
    button.innerText = "Run";
    button.addEventListener("click", () => {
      this.run();
    });
    p.appendChild(button);

    const out = document.createElement("div");
    out.classList.add("code-output");
    out.addEventListener("contextmenu", (ev) => {
      ev.preventDefault();
      this.output.innerText = "";
    });
    const outp = document.createElement("p");
    out.appendChild(outp);
    this.output = outp;
    p.appendChild(out);

    const extra = document.createElement("div");
    extra.classList.add("code-panel", "backpanel");

    this.panel = p;
    this.dom.appendChild(extra);
    this.dom.appendChild(this.panel);
    return p;
  }

  addLine(s: string = "") {
    const p = document.createElement("p");
    p.classList.add("code-line");
    p.innerText = s;
    this.back?.appendChild(p);
    this.domLines++;
  }

  keycheck(ev: KeyboardEvent) {
    clearTimeout(this.throttle);
    if (ev.code === "Enter") {
      if (ev.shiftKey) {
        ev.preventDefault();
        this.run();
      } else {
        setTimeout(() => {
          this.refreshCode();
        });
      }
    } else if (ev.code === "Tab") {
      ev.preventDefault();
      const start = this.area.selectionStart;
      const end = this.area.selectionEnd;
      this.area.value =
        this.area.value.substring(0, start) +
        "\t" +
        this.area.value.substring(end);
      this.area.selectionStart = this.area.selectionEnd = start + 1;
    } else {
      this.throttle = setTimeout(() => {
        this.refreshCode();
      }, 100);
    }
  }

  run() {
    const code = this.area.value;
    // this.output.innerText = "";
    if (this.runner) {
      const s = `> ${this.runner(code)}\n`;
      this.output.innerText += s;
    } else {
      this.output.innerText = "lua module failed to load";
    }
    if (this.output.parentElement)
      this.output.parentElement.scrollTop =
        this.output.parentElement.scrollHeight;
  }

  println(s: string) {
    this.output.innerText += `${s}\n`;
  }

  close(): void {
    let c = this.dom.childNodes.length;
    for (let i = 1; i < c; i++) {
      this.dom.removeChild(this.dom.childNodes[1]);
    }
  }
}

