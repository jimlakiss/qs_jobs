Rails.application.routes.draw do
  devise_for :users

  resources :projects
  resources :contributors

  root "projects#index"
end