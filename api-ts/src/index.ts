import app from './app.js'

const PORT = Number(process.env.PORT ?? 8081)

app.listen(PORT, () => {
  console.log(`agent-server listening on http://0.0.0.0:${PORT}`)
  console.log(`  API:     http://localhost:${PORT}/api/v1/agents`)
  console.log(`  Weather: http://localhost:${PORT}/api/v1/weather`)
  console.log(`  Web:     set NEXT_PUBLIC_CORAL_SERVER=http://localhost:${PORT} in web/.env.local`)
})
