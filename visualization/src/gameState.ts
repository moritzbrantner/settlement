import { buildRoute, type HexTile } from "./hex";

export type ResourceType = "wood" | "berries";
export type WorkerId = "worker-man" | "worker-woman";
export type TaskId =
  | "idle"
  | "gather-wood"
  | "gather-berries"
  | "build-fireplace"
  | "build-hut";

export type Worker = {
  id: WorkerId;
  label: string;
  gender: "Man" | "Woman";
  accent: "worker-man" | "worker-woman";
  tile: HexTile;
  home: HexTile;
  route: HexTile[];
  taskId: TaskId;
};

export type ResourceNode = {
  id: "wood-grove" | "berry-patch";
  label: string;
  resource: ResourceType;
  yieldPerTurn: number;
  tile: HexTile;
};

export type Blueprint = {
  id: "fireplace" | "hut";
  label: string;
  tile: HexTile;
  cost: Record<ResourceType, number>;
  requiredTurns: number;
  progress: number;
  built: boolean;
};

export type GameState = {
  turn: number;
  feedback: string;
  stockpile: Record<ResourceType, number>;
  workers: Worker[];
  resourceNodes: ResourceNode[];
  blueprints: Blueprint[];
};

type TaskDefinition = {
  id: TaskId;
  label: string;
  details: string;
};

const TASK_DEFINITIONS: TaskDefinition[] = [
  {
    id: "gather-wood",
    label: "Farm Wood",
    details: "Send the selected worker to the timber grove.",
  },
  {
    id: "gather-berries",
    label: "Farm Berries",
    details: "Send the selected worker to the berry patch.",
  },
  {
    id: "build-fireplace",
    label: "Build Fireplace",
    details: "Spend wood and berries to raise a village fire.",
  },
  {
    id: "build-hut",
    label: "Build Hut",
    details: "Spend wood to frame a permanent hut.",
  },
  {
    id: "idle",
    label: "Return Home",
    details: "Pull the worker back to the center of camp.",
  },
];

export function createInitialState(): GameState {
  const state: GameState = {
    turn: 1,
    feedback:
      "Select a worker, assign a job, then advance the turn to gather supplies or build.",
    stockpile: {
      wood: 2,
      berries: 1,
    },
    workers: [
      {
        id: "worker-man",
        label: "Rowan",
        gender: "Man",
        accent: "worker-man",
        tile: { q: -1, r: 0 },
        home: { q: -1, r: 0 },
        route: [],
        taskId: "idle",
      },
      {
        id: "worker-woman",
        label: "Mira",
        gender: "Woman",
        accent: "worker-woman",
        tile: { q: 1, r: 0 },
        home: { q: 1, r: 0 },
        route: [],
        taskId: "idle",
      },
    ],
    resourceNodes: [
      {
        id: "wood-grove",
        label: "Timber Grove",
        resource: "wood",
        yieldPerTurn: 2,
        tile: { q: -4, r: -1 },
      },
      {
        id: "berry-patch",
        label: "Berry Patch",
        resource: "berries",
        yieldPerTurn: 2,
        tile: { q: 4, r: -2 },
      },
    ],
    blueprints: [
      {
        id: "fireplace",
        label: "Fireplace",
        tile: { q: -2, r: 3 },
        cost: { wood: 4, berries: 2 },
        requiredTurns: 2,
        progress: 0,
        built: false,
      },
      {
        id: "hut",
        label: "Hut",
        tile: { q: 2, r: 3 },
        cost: { wood: 6, berries: 0 },
        requiredTurns: 3,
        progress: 0,
        built: false,
      },
    ],
  };

  assignTask(state, "worker-man", "gather-wood");
  assignTask(state, "worker-woman", "gather-berries");
  state.feedback =
    "Select a worker, assign a job, then advance the turn to gather supplies or build.";
  return state;
}

export function assignTask(state: GameState, workerId: WorkerId, taskId: TaskId) {
  const worker = state.workers.find((entry) => entry.id === workerId);
  if (!worker) {
    return;
  }

  worker.taskId = taskId;
  worker.route = buildRoute(worker.tile, getTaskTargetTile(state, worker));
  state.feedback = `${worker.gender} ${worker.label} is now assigned to ${getTaskLabel(taskId)}.`;
}

export function advanceTurn(state: GameState) {
  const events: string[] = [];

  for (const worker of state.workers) {
    const target = getTaskTargetTile(state, worker);
    worker.route = buildRoute(worker.tile, target);
    worker.tile = target;

    switch (worker.taskId) {
      case "gather-wood":
      case "gather-berries": {
        const node = getResourceNodeForTask(state, worker.taskId);
        if (!node) {
          break;
        }
        state.stockpile[node.resource] += node.yieldPerTurn;
        events.push(
          `${worker.label} gathered ${node.yieldPerTurn} ${node.resource} at the ${node.label}.`,
        );
        break;
      }
      case "build-fireplace":
      case "build-hut": {
        const blueprint = getBlueprintForTask(state, worker.taskId);
        if (!blueprint) {
          break;
        }
        if (blueprint.built) {
          worker.taskId = "idle";
          events.push(`${worker.label} found the ${blueprint.label.toLowerCase()} already finished.`);
          break;
        }

        const stepCost = getStepCost(blueprint);
        if (!canAfford(state.stockpile, stepCost)) {
          events.push(
            `${worker.label} could not continue the ${blueprint.label.toLowerCase()} because supplies are short.`,
          );
          break;
        }

        spendResources(state.stockpile, stepCost);
        blueprint.progress += 1;
        if (blueprint.progress >= blueprint.requiredTurns) {
          blueprint.built = true;
          worker.taskId = "idle";
          events.push(`${worker.label} completed the ${blueprint.label.toLowerCase()}.`);
        } else {
          events.push(
            `${worker.label} advanced the ${blueprint.label.toLowerCase()} to stage ${blueprint.progress}/${blueprint.requiredTurns}.`,
          );
        }
        break;
      }
      case "idle":
        events.push(`${worker.label} held position at camp.`);
        break;
    }

    worker.route = buildRoute(worker.tile, getTaskTargetTile(state, worker));
  }

  state.turn += 1;
  state.feedback = events.join(" ");
}

export function getTaskOptions(state: GameState) {
  return TASK_DEFINITIONS.map((task) => ({
    ...task,
    disabled:
      (task.id === "build-fireplace" && getBlueprint(state, "fireplace")?.built) ||
      (task.id === "build-hut" && getBlueprint(state, "hut")?.built) ||
      false,
  }));
}

export function getTaskLabel(taskId: TaskId) {
  return TASK_DEFINITIONS.find((task) => task.id === taskId)?.label ?? "Unknown task";
}

export function getTaskTargetTile(state: GameState, worker: Worker) {
  const node = getResourceNodeForTask(state, worker.taskId);
  if (node) {
    return node.tile;
  }

  const blueprint = getBlueprintForTask(state, worker.taskId);
  if (blueprint) {
    return blueprint.tile;
  }

  return worker.home;
}

export function describeWorker(state: GameState, workerId: WorkerId) {
  const worker = state.workers.find((entry) => entry.id === workerId);
  if (!worker) {
    return {
      title: "No worker selected",
      summary: "Choose Rowan or Mira on the board.",
      task: "",
    };
  }

  const target = getTaskTargetTile(state, worker);
  return {
    title: `${worker.gender}: ${worker.label}`,
    task: getTaskLabel(worker.taskId),
    summary:
      `Standing at (${worker.tile.q}, ${worker.tile.r}). ` +
      `Assigned to ${getTaskLabel(worker.taskId).toLowerCase()} toward (${target.q}, ${target.r}).`,
  };
}

export function describeBlueprint(blueprint: Blueprint) {
  if (blueprint.built) {
    return `${blueprint.label} stands complete at (${blueprint.tile.q}, ${blueprint.tile.r}).`;
  }

  const costs = formatCost(blueprint.cost);
  return (
    `${blueprint.label} needs ${costs}. ` +
    `Progress ${blueprint.progress}/${blueprint.requiredTurns}.`
  );
}

function getResourceNodeForTask(state: GameState, taskId: TaskId) {
  if (taskId === "gather-wood") {
    return state.resourceNodes.find((node) => node.resource === "wood");
  }
  if (taskId === "gather-berries") {
    return state.resourceNodes.find((node) => node.resource === "berries");
  }
  return undefined;
}

function getBlueprintForTask(state: GameState, taskId: TaskId) {
  if (taskId === "build-fireplace") {
    return getBlueprint(state, "fireplace");
  }
  if (taskId === "build-hut") {
    return getBlueprint(state, "hut");
  }
  return undefined;
}

function getBlueprint(state: GameState, blueprintId: Blueprint["id"]) {
  return state.blueprints.find((entry) => entry.id === blueprintId);
}

function getStepCost(blueprint: Blueprint) {
  return {
    wood: blueprint.cost.wood / blueprint.requiredTurns,
    berries: blueprint.cost.berries / blueprint.requiredTurns,
  };
}

function canAfford(
  stockpile: Record<ResourceType, number>,
  cost: Record<ResourceType, number>,
) {
  return (
    stockpile.wood + Number.EPSILON >= cost.wood &&
    stockpile.berries + Number.EPSILON >= cost.berries
  );
}

function spendResources(
  stockpile: Record<ResourceType, number>,
  cost: Record<ResourceType, number>,
) {
  stockpile.wood = Math.max(0, stockpile.wood - cost.wood);
  stockpile.berries = Math.max(0, stockpile.berries - cost.berries);
}

function formatCost(cost: Record<ResourceType, number>) {
  return [`${cost.wood} wood`, `${cost.berries} berries`]
    .filter((entry) => !entry.startsWith("0 "))
    .join(" and ");
}
