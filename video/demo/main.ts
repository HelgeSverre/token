/**
 * Token Editor — Application Server
 * A fast, minimal text editor built with Rust.
 */

import { serve } from "bun";
import { Database } from "./database";
import { Logger } from "./logger";

// Configuration
const config = {
  port: 3000,
  host: "localhost",
  database: "sqlite:///data/app.db",
  logLevel: "info",
} as const;

// Types
interface User {
  id: number;
  name: string;
  email: string;
  role: "admin" | "editor" | "viewer";
  created: Date;
}

interface ApiResponse<T> {
  data: T;
  status: number;
  message: string;
}

// Initialize services
const logger = new Logger(config.logLevel);
const database = new Database(config.database);

// Route handlers
async function getUsers(): Promise<ApiResponse<User[]>> {
  const users = await database.query<User>("SELECT * FROM users");
  logger.info(`Fetched ${users.length} users`);
  return { data: users, status: 200, message: "ok" };
}

async function getUser(id: number): Promise<ApiResponse<User>> {
  const user = await database.findOne<User>("users", id);
  if (!user) {
    return { data: null as any, status: 404, message: "not found" };
  }
  return { data: user, status: 200, message: "ok" };
}

async function createUser(body: Partial<User>): Promise<ApiResponse<User>> {
  const user = await database.insert<User>("users", {
    name: body.name!,
    email: body.email!,
    role: body.role ?? "viewer",
    created: new Date(),
  });
  logger.info(`Created user: ${user.name}`);
  return { data: user, status: 201, message: "created" };
}

async function updateUser(id: number, body: Partial<User>): Promise<ApiResponse<User>> {
  const user = await database.update<User>("users", id, body);
  logger.info(`Updated user: ${user.name}`);
  return { data: user, status: 200, message: "updated" };
}

async function deleteUser(id: number): Promise<ApiResponse<null>> {
  await database.delete("users", id);
  logger.info(`Deleted user: ${id}`);
  return { data: null, status: 200, message: "deleted" };
}

// Server
const server = serve({
  port: config.port,
  hostname: config.host,

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url);
    const path = url.pathname;

    try {
      if (path === "/api/users" && request.method === "GET") {
        const result = await getUsers();
        return Response.json(result);
      }

      if (path.startsWith("/api/users/") && request.method === "GET") {
        const id = parseInt(path.split("/").pop()!);
        const result = await getUser(id);
        return Response.json(result, { status: result.status });
      }

      if (path === "/api/users" && request.method === "POST") {
        const body = await request.json();
        const result = await createUser(body);
        return Response.json(result, { status: 201 });
      }

      return Response.json(
        { error: "not found", status: 404 },
        { status: 404 }
      );
    } catch (error) {
      logger.error(`Request failed: ${error}`);
      return Response.json(
        { error: "internal server error", status: 500 },
        { status: 500 }
      );
    }
  },
});

logger.info(`Server running at http://${config.host}:${config.port}`);
