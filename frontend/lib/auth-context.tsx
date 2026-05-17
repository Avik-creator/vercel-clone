"use client"

import React, { createContext, useContext, useEffect, useState, useCallback } from "react"
import { api, User } from "@/lib/api"

interface AuthContextType {
  user: User | null
  isLoading: boolean
  isAuthenticated: boolean
  login: (email: string, password: string) => Promise<void>
  register: (email: string, name: string, password: string) => Promise<void>
  logout: () => void
  setUser: (user: User) => void
  refreshUser: () => Promise<void>
}

const AuthContext = createContext<AuthContextType | undefined>(undefined)

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [isLoading, setIsLoading] = useState(true)

  const refreshUser = useCallback(async () => {
    const token = api.getToken()
    if (!token) {
      setUser(null)
      setIsLoading(false)
      return
    }

    try {
      const userData = await api.getMe()
      setUser(userData)
      localStorage.setItem("auth_user", JSON.stringify(userData))
    } catch {
      // Token might be expired or invalid
      api.logout()
      setUser(null)
      localStorage.removeItem("auth_user")
    }
    setIsLoading(false)
  }, [])

  useEffect(() => {
    // Check if user is logged in
    const token = api.getToken()
    if (token) {
      // First try to use cached user data for immediate display
      const storedUser = localStorage.getItem("auth_user")
      if (storedUser) {
        try {
          setUser(JSON.parse(storedUser))
        } catch {
          // Invalid stored data, will refresh from API
        }
      }
      // Then refresh from API to ensure data is current
      refreshUser()
    } else {
      setIsLoading(false)
    }
  }, [refreshUser])

  const login = useCallback(async (email: string, password: string) => {
    const response = await api.login({ email, password })
    setUser(response.user)
    localStorage.setItem("auth_user", JSON.stringify(response.user))
  }, [])

  const register = useCallback(async (email: string, name: string, password: string) => {
    const response = await api.register({ email, name, password })
    setUser(response.user)
    localStorage.setItem("auth_user", JSON.stringify(response.user))
  }, [])

  const logout = useCallback(() => {
    api.logout()
    setUser(null)
    localStorage.removeItem("auth_user")
  }, [])

  return (
    <AuthContext.Provider
      value={{
        user,
        isLoading,
        isAuthenticated: !!user,
        login,
        register,
        logout,
        setUser,
        refreshUser,
      }}
    >
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth() {
  const context = useContext(AuthContext)
  if (context === undefined) {
    throw new Error("useAuth must be used within an AuthProvider")
  }
  return context
}
