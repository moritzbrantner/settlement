import "./styles.css";
import "./board.css";

import {
  advanceTurn,
  assignTask,
  createInitialState,
  describeBlueprint,
  describeWorker,
  getTaskOptions,
  type WorkerId,
} from "./gameState";
import { installHotkeys, setButtonLabelWithHotkey } from "./hotkeys";
import {
  buildCenters,
  buildHexagon,
  MAP_RADIUS,
  measureBounds,
  round,
  tileKey,
  toPercent,
} from "./hex";

const CORE_DISTANCE = 1;
const WORKER_HOTKEYS: WorkerId[] = ["worker-man", "worker-woman"];
const WORKER_SELECTION_HOTKEYS = ["q", "w"];
const TASK_HOTKEYS = ["1", "2", "3", "4", "5"];
const ADVANCE_TURN_HOTKEY = "space";

const app = {
  map: queryRequired<SVGElement>("#hex-map"),
  featureLayer: queryRequired<HTMLDivElement>("#feature-layer"),
  workerLayer: queryRequired<HTMLDivElement>("#worker-layer"),
  tileCount: queryRequired<HTMLElement>("#stat-tile-count"),
  workerCount: queryRequired<HTMLElement>("#stat-worker-count"),
  turnCount: queryRequired<HTMLElement>("#stat-turn-count"),
  woodCount: queryRequired<HTMLElement>("#stat-wood-count"),
  berryCount: queryRequired<HTMLElement>("#stat-berry-count"),
  workerName: queryRequired<HTMLElement>("#worker-name"),
  workerTask: queryRequired<HTMLElement>("#worker-task"),
  workerSummary: queryRequired<HTMLElement>("#worker-summary"),
  taskButtons: queryRequired<HTMLDivElement>("#task-buttons"),
  constructionList: queryRequired<HTMLDivElement>("#construction-list"),
  feedback: queryRequired<HTMLElement>("#feedback-message"),
  advanceTurn: queryRequired<HTMLButtonElement>("#advance-turn"),
};

const state = createInitialState();
const tiles = buildHexagon(MAP_RADIUS);
const centers = buildCenters(tiles);
const bounds = measureBounds(centers);
let selectedWorkerId: WorkerId = state.workers[0].id;

renderMap();
renderAll();
configureHotkeyLabels();
installHotkeys(settlementHotkeyBindings);

app.advanceTurn.addEventListener("click", () => {
  advanceTurn(state);
  renderAll();
});

function renderAll() {
  renderStats();
  renderFeatures();
  renderWorkers();
  renderSidebar();
  renderTaskButtons();
  renderConstruction();
  renderSelectedWorkerRoute();
}

function renderMap() {
  app.map.setAttribute("viewBox", `${bounds.minX} ${bounds.minY} ${bounds.width} ${bounds.height}`);

  const resourceTiles = new Set(state.resourceNodes.map((node) => tileKey(node.tile)));
  const blueprintTiles = new Set(state.blueprints.map((blueprint) => tileKey(blueprint.tile)));

  const tileMarkup = centers
    .map((tile) => {
      const classes = ["hex-tile"];
      if (tile.distance <= CORE_DISTANCE) {
        classes.push("hex-tile-core");
      }
      if (resourceTiles.has(tile.key)) {
        classes.push("hex-tile-resource");
      }
      if (blueprintTiles.has(tile.key)) {
        classes.push("hex-tile-blueprint");
      }

      return `
        <polygon
          class="${classes.join(" ")}"
          data-tile="${tile.key}"
          points="${tile.corners}"
        />
      `;
    })
    .join("");

  app.map.innerHTML = `
    <defs>
      <radialGradient id="boardGlow" cx="50%" cy="48%" r="70%">
        <stop offset="0%" stop-color="#f5d58e" stop-opacity="0.28" />
        <stop offset="100%" stop-color="#f5d58e" stop-opacity="0" />
      </radialGradient>
    </defs>
    <rect
      class="board-glow"
      x="${bounds.minX}"
      y="${bounds.minY}"
      width="${bounds.width}"
      height="${bounds.height}"
      fill="url(#boardGlow)"
      rx="36"
    />
    ${tileMarkup}
    <g id="route-layer" class="route-layer"></g>
  `;
}

function renderStats() {
  app.tileCount.textContent = centers.length.toString();
  app.workerCount.textContent = state.workers.length.toString();
  app.turnCount.textContent = state.turn.toString();
  app.woodCount.textContent = state.stockpile.wood.toFixed(0);
  app.berryCount.textContent = state.stockpile.berries.toFixed(0);
}

function renderFeatures() {
  app.featureLayer.innerHTML = "";

  for (const node of state.resourceNodes) {
    app.featureLayer.append(
      createBoardToken({
        tile: node.tile,
        className: `feature-token feature-${node.resource}`,
        title: `${node.label} produces ${node.yieldPerTurn} ${node.resource} each turn`,
        sigil: node.resource === "wood" ? "W" : "B",
        label: node.label,
      }),
    );
  }

  for (const blueprint of state.blueprints) {
    app.featureLayer.append(
      createBoardToken({
        tile: blueprint.tile,
        className: `feature-token feature-build ${blueprint.built ? "is-built" : "is-pending"}`,
        title: describeBlueprint(blueprint),
        sigil: blueprint.id === "fireplace" ? "F" : "H",
        label: blueprint.built ? blueprint.label : `${blueprint.label} Site`,
      }),
    );
  }
}

function renderWorkers() {
  app.workerLayer.innerHTML = "";

  for (const [index, worker] of state.workers.entries()) {
    const button = createBoardToken({
      tile: worker.tile,
      className: `worker-token ${worker.accent}`,
      title: `${worker.gender} ${worker.label} at ${worker.tile.q}, ${worker.tile.r} (${WORKER_SELECTION_HOTKEYS[index]?.toUpperCase() ?? ""})`,
      sigil: worker.gender === "Man" ? "M" : "W",
      label: worker.label,
      asButton: true,
    });

    button.dataset.workerId = worker.id;
    if (WORKER_SELECTION_HOTKEYS[index]) {
      button.setAttribute("aria-keyshortcuts", WORKER_SELECTION_HOTKEYS[index].toUpperCase());
    }
    button.setAttribute("aria-pressed", worker.id === selectedWorkerId ? "true" : "false");
    button.classList.toggle("is-selected", worker.id === selectedWorkerId);
    button.addEventListener("click", () => {
      selectedWorkerId = worker.id;
      renderAll();
    });
    app.workerLayer.append(button);
  }
}

function renderSidebar() {
  const details = describeWorker(state, selectedWorkerId);
  app.workerName.textContent = details.title;
  app.workerTask.textContent = details.task;
  app.workerSummary.textContent = details.summary;
  app.feedback.textContent = state.feedback;
}

function renderTaskButtons() {
  app.taskButtons.innerHTML = "";

  for (const [index, task] of getTaskOptions(state).entries()) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "task-button";
    if (selectedWorker().taskId === task.id) {
      button.classList.add("is-active");
    }
    button.disabled = task.disabled;
    const hotkey = TASK_HOTKEYS[index] ?? null;
    if (hotkey) {
      button.setAttribute("aria-keyshortcuts", hotkey.toUpperCase());
    }
    const labelRow = document.createElement("span");
    labelRow.className = "task-button-label";
    setButtonLabelWithHotkey(labelRow, task.label, hotkey);
    const details = document.createElement("span");
    details.className = "task-button-copy";
    details.textContent = task.details;
    button.append(labelRow, details);
    button.addEventListener("click", () => {
      assignTask(state, selectedWorkerId, task.id);
      renderAll();
    });
    app.taskButtons.append(button);
  }
}

function renderConstruction() {
  app.constructionList.innerHTML = "";

  for (const blueprint of state.blueprints) {
    const article = document.createElement("article");
    article.className = "construction-item";
    article.innerHTML = `
      <div>
        <p class="construction-name">${blueprint.label}</p>
        <p class="construction-copy">${describeBlueprint(blueprint)}</p>
      </div>
      <span class="construction-badge ${blueprint.built ? "is-built" : "is-pending"}">
        ${blueprint.built ? "Built" : `${blueprint.progress}/${blueprint.requiredTurns}`}
      </span>
    `;
    app.constructionList.append(article);
  }
}

function renderSelectedWorkerRoute() {
  const routeLayer = app.map.querySelector("#route-layer");
  const worker = selectedWorker();

  if (!routeLayer || worker.route.length < 2) {
    if (routeLayer) {
      routeLayer.innerHTML = "";
    }
    return;
  }

  const routePoints = worker.route.map(findTileCenter);
  const pathData = routePoints
    .map((point, index) => `${index === 0 ? "M" : "L"} ${round(point.x)} ${round(point.y)}`)
    .join(" ");
  const destination = routePoints[routePoints.length - 1];

  routeLayer.innerHTML = `
    <path class="worker-route-shadow" d="${pathData}" />
    <path class="worker-route" d="${pathData}" />
    <circle
      class="worker-route-target"
      cx="${round(destination.x)}"
      cy="${round(destination.y)}"
      r="6"
    />
  `;
}

function createBoardToken(options: {
  tile: { q: number; r: number };
  className: string;
  title: string;
  sigil: string;
  label: string;
  asButton?: boolean;
}) {
  const center = findTileCenter(options.tile);
  const element = document.createElement(options.asButton ? "button" : "div");
  element.className = options.className;
  element.style.left = `${toPercent(center.x, bounds.minX, bounds.width)}%`;
  element.style.top = `${toPercent(center.y, bounds.minY, bounds.height)}%`;
  element.setAttribute("title", options.title);
  if (options.asButton) {
    (element as HTMLButtonElement).type = "button";
  }
  element.innerHTML = `
    <span class="token-sigil" aria-hidden="true">${options.sigil}</span>
    <span class="token-label">${options.label}</span>
  `;
  return element;
}

function selectedWorker() {
  return state.workers.find((worker) => worker.id === selectedWorkerId) ?? state.workers[0];
}

function configureHotkeyLabels() {
  setButtonLabelWithHotkey(app.advanceTurn, "Advance Turn", ADVANCE_TURN_HOTKEY);
}

function settlementHotkeyBindings() {
  return [
    ...WORKER_HOTKEYS.map((workerId, index) => ({
      key: WORKER_SELECTION_HOTKEYS[index],
      run: () => {
        selectedWorkerId = workerId;
        renderAll();
      },
    })),
    ...getTaskOptions(state).map((task, index) => ({
      key: TASK_HOTKEYS[index],
      enabled: () => !task.disabled,
      run: () => {
        assignTask(state, selectedWorkerId, task.id);
        renderAll();
      },
    })),
    {
      key: ADVANCE_TURN_HOTKEY,
      run: () => {
        advanceTurn(state);
        renderAll();
      },
    },
  ];
}

function findTileCenter(tile: { q: number; r: number }) {
  const center = centers.find((entry) => entry.q === tile.q && entry.r === tile.r);
  if (!center) {
    throw new Error(`Missing tile for ${tile.q},${tile.r}`);
  }
  return center;
}

function queryRequired<ElementType extends Element>(selector: string) {
  const element = document.querySelector<ElementType>(selector);
  if (!element) {
    throw new Error(`Missing required element: ${selector}`);
  }
  return element;
}
