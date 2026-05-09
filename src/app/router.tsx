import {
  createRootRoute,
  createRoute,
  createRouter,
} from "@tanstack/react-router";

import { AppShell } from "@/app/AppShell";
import { CollectionRoute } from "@/app/routes/collection-route";
import { HomeRoute } from "@/app/routes/home-route";

const rootRoute = createRootRoute({
  component: AppShell,
});

const indexRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/",
  component: HomeRoute,
});

const collectionRoute = createRoute({
  getParentRoute: () => rootRoute,
  path: "/collections/$collectionId",
  component: CollectionRoute,
});

const routeTree = rootRoute.addChildren([indexRoute, collectionRoute]);

export const router = createRouter({ routeTree });

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router;
  }
}
