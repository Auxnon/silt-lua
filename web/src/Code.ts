import "./index.scss";
import init, { run } from "silt-lua";

interface LSPOutput {
  legend: string[];
  map: [number, number, number][];
  indented: string;
}

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

export default class Code {
  panel: HTMLElement;
  area: HTMLTextAreaElement;
  highlight: HTMLElement;
  back: HTMLElement;
  output: HTMLElement;
  lines: number = 0;
  domLines: number = 0;
  runner?: (s: string) => string;
  lsp?: (s: string, format: boolean) => string;
  throttle: any;

  constructor(private dom: HTMLElement, id: number) {
    this.resolver();
    init().then(() => {
      // @ts-ignore
      this.runner = (s) => run(s);
      // @ts-ignore
      this.lsp = (s, format) => window.wasm_bindgen.lsp(s, format);
      // @ts-ignore
      window.jprintln = (s) => this.println(s);
    });
    this.makePanel();
    this.refreshCode();
  }

  resolver() {
    // Placeholder for resolver functionality
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
    const fontSize = 32;
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
    this.area.style.height = this.lines * fontSize + "px";
    this.highlight.style.height = this.lines * fontSize + "px";
    this.updateHighlighting();
  }

  updateHighlighting() {
    if (!this.lsp) return;
    
    try {
      const lspResult = this.lsp(this.area.value, false);
      const parsed: LSPOutput = JSON.parse(lspResult);
      
      let highlightedText = this.area.value;
      const tokens: Array<{start: number, length: number, type: number}> = [];
      
      // Sort tokens by position to apply highlighting correctly
      parsed.map.forEach(([start, length, type]) => {
        tokens.push({start, length, type});
      });
      tokens.sort((a, b) => a.start - b.start);
      
      // Apply syntax highlighting
      let offset = 0;
      tokens.forEach(token => {
        const start = token.start + offset;
        const end = start + token.length;
        const className = this.getTokenClass(token.type);
        
        if (className) {
          const before = highlightedText.substring(0, start);
          const tokenText = highlightedText.substring(start, end);
          const after = highlightedText.substring(end);
          
          const wrapped = `<span class="${className}">${this.escapeHtml(tokenText)}</span>`;
          highlightedText = before + wrapped + after;
          offset += wrapped.length - tokenText.length;
        }
      });
      
      this.highlight.innerHTML = this.escapeHtml(highlightedText.substring(0, this.area.value.length)) + 
                                highlightedText.substring(this.area.value.length);
    } catch (e) {
      // Fallback to plain text if LSP fails
      this.highlight.textContent = this.area.value;
    }
  }

  getTokenClass(type: number): string {
    const classes = ['', 'keyword', 'operator', 'number', 'boolean', 'nil', 'string', 'comment'];
    return classes[type] || '';
  }

  escapeHtml(text: string): string {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
  }

  syncScroll() {
    this.highlight.scrollTop = this.area.scrollTop;
    this.highlight.scrollLeft = this.area.scrollLeft;
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
    this.area.addEventListener("input", () => {
      this.updateHighlighting();
    });
    this.area.addEventListener("scroll", () => {
      this.syncScroll();
    });
    seg.appendChild(a);

    const h = document.createElement("div");
    h.classList.add("code-highlight");
    this.highlight = h;
    seg.appendChild(h);

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
        }, 0);
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
