#!/usr/bin/env bash
cd frontend && trunk serve --public-url salsa/ --proxy-backend http://localhost:3000/api/ --proxy-rewrite /api/
