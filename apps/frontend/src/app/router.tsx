import { createBrowserRouter, Navigate } from "react-router-dom";
import { AppLayout } from "../features/layout/AppLayout";
import { DetectionPage } from "../features/detection/DetectionPage";
import { DownloadsPage } from "../features/downloads/DownloadsPage";
import { HistoryPage } from "../features/history/HistoryPage";
import { SettingsPage } from "../features/settings/SettingsPage";

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <Navigate to="/detect" replace /> },
      { path: "detect", element: <DetectionPage /> },
      { path: "downloads", element: <DownloadsPage /> },
      { path: "history", element: <HistoryPage /> },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);
